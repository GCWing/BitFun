package com.bitfun.mobile.core.feature.workspace

import com.bitfun.mobile.core.domain.FilePreviewFailure
import com.bitfun.mobile.core.domain.FilePreviewFailureReason
import com.bitfun.mobile.core.domain.FilePreviewPolicy
import com.bitfun.mobile.core.domain.FilePreviewRenderer
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
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlin.io.encoding.Base64

public class RemoteWorkspaceStore internal constructor(
    private val scope: CoroutineScope,
    private val transport: RemoteCommandTransport,
    private val backgroundDispatcher: CoroutineDispatcher,
    public val deviceKey: String? = null,
) {
    private val _state = MutableStateFlow<RemoteWorkspaceUiState>(RemoteWorkspaceUiState.Idle)
    public val state: StateFlow<RemoteWorkspaceUiState> = _state.asStateFlow()
    private val _stopVersion = MutableStateFlow(0L)
    /** Changes only when this target store is stopped; useful to cancel observers. */
    public val stopVersion: StateFlow<Long> = _stopVersion.asStateFlow()
    private var work: Job? = null
    private var targetEpoch: Int = 0
    private var previewGeneration: Long = 0
    private var activePreviewRequestId: String? = null

    private fun nextPreviewIdentity(target: FilePreviewTarget, requestedId: String = ""): PreviewRequestIdentity {
        previewGeneration += 1
        val requestId = requestedId.trim().ifEmpty { "preview-$previewGeneration" }
        activePreviewRequestId = requestId
        return PreviewRequestIdentity(requestId, deviceKey, target.sessionId, target.remotePath)
    }

    private fun invalidatePreview() {
        previewGeneration += 1
        activePreviewRequestId = null
    }

    public fun dispatch(intent: RemoteWorkspaceIntent) {
        when (intent) {
            RemoteWorkspaceIntent.Load -> load()
            is RemoteWorkspaceIntent.SelectWorkspace -> selectWorkspace(intent.path)
            is RemoteWorkspaceIntent.SelectAssistant -> selectAssistant(intent.path)
            is RemoteWorkspaceIntent.OpenFile -> resolveAndOpenFile(intent)
            is RemoteWorkspaceIntent.DownloadFile -> resolveAndDownloadFile(intent)
            is RemoteWorkspaceIntent.DownloadSaved -> finishDownload(intent.reference, true)
            is RemoteWorkspaceIntent.DownloadSaveFailed -> finishDownload(intent.reference, false)
            RemoteWorkspaceIntent.DismissPreview -> {
                invalidatePreview()
                updateReady { it.copy(preview = RemoteFilePreviewUiState.None) }
            }
            RemoteWorkspaceIntent.Stop -> stop()
        }
    }

    public fun stop() {
        _stopVersion.value += 1
        invalidatePreview()
        work?.cancel()
        work = null
    }

    private fun load() {
        invalidatePreview()
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
                    download = RemoteFileDownloadUiState.None,
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
        invalidatePreview()
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

    private fun openFile(target: FilePreviewTarget, requestedId: String) {
        val current = _state.value as? RemoteWorkspaceUiState.Ready ?: return
        val identity = nextPreviewIdentity(target, requestedId)
        val generation = previewGeneration
        work?.cancel()
        _state.value = current.copy(preview = RemoteFilePreviewUiState.Loading(target, identity))
        work = scope.launch {
            try {
                val info = transport.send<FileInfoResponse>(
                    RemoteCommand(cmd = "get_file_info", path = target.remotePath, sessionId = target.sessionId.ifEmpty { null }),
                )
                val size = info.size ?: 0
                val mime = info.mimeType ?: "application/octet-stream"
                val name = info.name ?: basename(target.remotePath)
                // The name decides as much as the type does: the desktop reports
                // `text/plain` for Markdown, and `image/svg+xml` for a file the
                // preview can only show as source.
                when (FilePreviewPolicy.rendererFor(name.ifEmpty { target.remotePath }, mime)) {
                    FilePreviewRenderer.MARKDOWN -> loadText(target, identity, generation, name, mime, size, markdown = true)
                    FilePreviewRenderer.TEXT -> loadText(target, identity, generation, name, mime, size, markdown = false)
                    FilePreviewRenderer.IMAGE ->
                        if (FilePreviewPolicy.canPreviewImage(size)) {
                            loadImage(target, identity, generation, name, mime, size)
                        } else {
                            failPreview(target, identity, generation, "file too large", mime, size)
                        }
                    FilePreviewRenderer.UNSUPPORTED ->
                        updatePreview(identity, generation) { it.copy(preview = RemoteFilePreviewUiState.Unsupported(target, mime, size, identity)) }
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                // `get_file_info` may be what failed, so the type and size are
                // not known here; the header falls back to the path it asked for.
                failPreview(target, identity, generation, error.message.orEmpty(), "", 0)
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
                        "",
                        0,
                    ),
                )
            }
            return
        }
        openFile(resolvedTarget, intent.requestId)
    }

    private fun resolveAndDownloadFile(intent: RemoteWorkspaceIntent.DownloadFile) {
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
        val target = resolution.target
        if (resolution.kind != FileReferenceKind.REMOTE_WORKSPACE_FILE || target == null) {
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
                    download = RemoteFileDownloadUiState.Failed(
                        placeholder,
                        FilePreviewFailureKind.UNAVAILABLE,
                        false,
                    ),
                )
            }
            return
        }
        downloadFile(target)
    }

    private fun downloadFile(target: FilePreviewTarget) {
        val current = _state.value as? RemoteWorkspaceUiState.Ready ?: return
        if (current.busy || current.download is RemoteFileDownloadUiState.Loading ||
            current.download is RemoteFileDownloadUiState.AwaitingSave
        ) return
        work?.cancel()
        _state.value = current.copy(download = RemoteFileDownloadUiState.Loading(target, 0, 0))
        work = scope.launch {
            try {
                val info = transport.send<FileInfoResponse>(
                    RemoteCommand(
                        cmd = "get_file_info",
                        path = target.remotePath,
                        sessionId = target.sessionId.ifEmpty { null },
                    ),
                )
                val total = (info.size ?: 0).coerceAtLeast(0)
                val chunks = mutableListOf<ByteArray>()
                var offset = 0
                var expectedTotal = total
                var name = info.name ?: basename(target.remotePath)
                var mime = info.mimeType ?: "application/octet-stream"
                updateReady { it.copy(download = RemoteFileDownloadUiState.Loading(target, 0, total)) }
                do {
                    val response = transport.send<ReadFileChunkResponse>(
                        RemoteCommand(
                            cmd = "read_file_chunk",
                            path = target.remotePath,
                            sessionId = target.sessionId.ifEmpty { null },
                            offset = offset,
                            limit = DOWNLOAD_CHUNK_BYTES,
                        ),
                    )
                    val bytes = withContext(backgroundDispatcher) { decode(response.chunkBase64.orEmpty()) }
                    expectedTotal = (response.totalSize ?: expectedTotal).coerceAtLeast(offset.toLong())
                    if (bytes.isEmpty() && offset.toLong() < expectedTotal) {
                        error("remote file transfer stopped before completion")
                    }
                    chunks += bytes
                    offset += bytes.size
                    name = response.name?.takeIf(String::isNotBlank) ?: name
                    mime = response.mimeType?.takeIf(String::isNotBlank) ?: mime
                    val responseTotal = expectedTotal.coerceAtLeast(offset.toLong())
                    updateReady {
                        it.copy(download = RemoteFileDownloadUiState.Loading(target, offset.toLong(), responseTotal))
                    }
                    if (offset == Int.MAX_VALUE && offset.toLong() < expectedTotal) {
                        error("remote file is too large for this client")
                    }
                } while (offset.toLong() < expectedTotal)
                val bytes = withContext(backgroundDispatcher) { chunks.joinBytes() }
                updateReady {
                    it.copy(download = RemoteFileDownloadUiState.AwaitingSave(target, name, mime, bytes))
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                failDownload(target, error.message.orEmpty())
            }
        }
    }

    private fun finishDownload(reference: String, saved: Boolean) {
        val current = (_state.value as? RemoteWorkspaceUiState.Ready)?.download
            as? RemoteFileDownloadUiState.AwaitingSave ?: return
        if (reference != current.target.path && reference != current.target.remotePath) return
        updateReady {
            it.copy(
                download = if (saved) {
                    RemoteFileDownloadUiState.Saved(current.target, current.name)
                } else {
                    RemoteFileDownloadUiState.Failed(current.target, FilePreviewFailureKind.LOAD_FAILED, true)
                },
            )
        }
    }

    private fun failDownload(target: FilePreviewTarget, message: String) {
        val failure = if (message.isBlank()) {
            FilePreviewFailure(FilePreviewFailureReason.LOAD_FAILED, true)
        } else {
            FilePreviewPolicy.failure(message)
        }
        updateReady {
            it.copy(download = RemoteFileDownloadUiState.Failed(target, failure.toKind(), failure.retryable))
        }
    }

    private suspend fun loadText(
        target: FilePreviewTarget,
        identity: PreviewRequestIdentity,
        generation: Long,
        name: String,
        mime: String,
        size: Long,
        markdown: Boolean,
    ) {
        val limit = FilePreviewPolicy.textReadLimit(size).coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
        val response = readChunk(target, limit)
        val bytes = withContext(backgroundDispatcher) { decode(response.chunkBase64.orEmpty()) }
        val content = withContext(backgroundDispatcher) { bytes.decodeToString() }
        // The type said text; the bytes are the only thing that can disagree,
        // and a wall of replacement characters is worse than saying no.
        if (FilePreviewPolicy.looksBinary(bytes) || FilePreviewPolicy.looksUndecodable(bytes, content)) {
            updatePreview(identity, generation) {
                it.copy(
                    preview = RemoteFilePreviewUiState.Unsupported(
                        target,
                        response.mimeType ?: mime,
                        response.totalSize ?: size,
                        identity,
                    ),
                )
            }
            return
        }
        updatePreview(identity, generation) {
            it.copy(
                preview = RemoteFilePreviewUiState.Text(
                    target = target,
                    name = response.name ?: name,
                    content = content,
                    truncated = (response.totalSize ?: size) > bytes.size,
                    loadedBytes = bytes.size.toLong(),
                    mimeType = response.mimeType ?: mime,
                    sizeBytes = response.totalSize ?: size,
                    markdown = markdown,
                    identity = identity,
                ),
            )
        }
    }

    private suspend fun loadImage(target: FilePreviewTarget, identity: PreviewRequestIdentity, generation: Long, name: String, mime: String, size: Long) {
        val response = readChunk(target, size.coerceAtLeast(1).coerceAtMost(Int.MAX_VALUE.toLong()).toInt())
        val bytes = withContext(backgroundDispatcher) { decode(response.chunkBase64.orEmpty()) }
        updatePreview(identity, generation) {
            it.copy(
                preview = RemoteFilePreviewUiState.Image(
                    target = target,
                    name = response.name ?: name,
                    mimeType = response.mimeType ?: mime,
                    bytes = bytes,
                    sizeBytes = response.totalSize ?: size,
                    identity = identity,
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

    private fun failPreview(target: FilePreviewTarget, identity: PreviewRequestIdentity, generation: Long, message: String, mime: String, size: Long) {
        val failure = if (message.isBlank()) {
            FilePreviewFailure(FilePreviewFailureReason.LOAD_FAILED, true)
        } else {
            FilePreviewPolicy.failure(message)
        }
        updatePreview(identity, generation) {
            it.copy(
                preview = RemoteFilePreviewUiState.Failed(target, failure.toKind(), failure.retryable, mime, size, identity),
            )
        }
    }

    private fun updatePreview(
        identity: PreviewRequestIdentity,
        generation: Long,
        transform: (RemoteWorkspaceUiState.Ready) -> RemoteWorkspaceUiState.Ready,
    ) {
        if (previewGeneration != generation || activePreviewRequestId != identity.requestId) return
        updateReady(transform)
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

    private fun decode(value: String): ByteArray = Base64.Default.decode(value)

    private fun List<ByteArray>.joinBytes(): ByteArray {
        val result = ByteArray(sumOf { it.size })
        var offset = 0
        forEach { chunk ->
            chunk.copyInto(result, offset)
            offset += chunk.size
        }
        return result
    }

    private fun basename(path: String): String = path.replace('\\', '/').substringAfterLast('/').ifEmpty { "file" }

    public companion object {
        internal fun create(scope: CoroutineScope, transport: RemoteCommandTransport): RemoteWorkspaceStore =
            RemoteWorkspaceStore(scope, transport, Dispatchers.Default)

        internal fun create(
            scope: CoroutineScope,
            transport: RemoteCommandTransport,
            backgroundDispatcher: CoroutineDispatcher,
        ): RemoteWorkspaceStore = RemoteWorkspaceStore(scope, transport, backgroundDispatcher)

        internal fun create(
            scope: CoroutineScope,
            transport: RemoteCommandTransport,
            backgroundDispatcher: CoroutineDispatcher,
            deviceKey: String,
        ): RemoteWorkspaceStore = RemoteWorkspaceStore(scope, transport, backgroundDispatcher, deviceKey)

        private const val DOWNLOAD_CHUNK_BYTES = 3 * 1024 * 1024
    }
}
