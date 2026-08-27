package com.bitfun.mobile.core.feature.session

import com.bitfun.mobile.core.domain.ChatMessage
import com.bitfun.mobile.core.domain.ChatSessionController
import com.bitfun.mobile.core.domain.ChatSessionControllerCallbacks
import com.bitfun.mobile.core.domain.ChatSessionCursor
import com.bitfun.mobile.core.domain.ChatSessionPoller
import com.bitfun.mobile.core.domain.ChatSessionSnapshot
import com.bitfun.mobile.core.domain.ChatTimelineStore
import com.bitfun.mobile.core.domain.PollSessionResult
import com.bitfun.mobile.core.persistence.MobilePersistenceStores
import com.bitfun.mobile.core.persistence.PersistedRemoteCursor
import com.bitfun.mobile.core.persistence.PersistedRemoteMessage
import com.bitfun.mobile.core.persistence.PersistedRemoteSession
import com.bitfun.mobile.core.domain.RemoteSession
import com.bitfun.mobile.core.domain.SessionNaming
import com.bitfun.mobile.core.domain.SessionAgentTypes
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.protocol.ActiveTurnSnapshotResponse
import com.bitfun.mobile.core.protocol.ChatMessageItemResponse
import com.bitfun.mobile.core.protocol.ChatMessageResponse
import com.bitfun.mobile.core.protocol.CreateSessionResponse
import com.bitfun.mobile.core.protocol.InitialSyncResponse
import com.bitfun.mobile.core.protocol.ImageAttachment
import com.bitfun.mobile.core.protocol.ModelCatalogResponse
import com.bitfun.mobile.core.protocol.RemoteToolStatusResponse
import com.bitfun.mobile.core.protocol.PollSessionResponse
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.protocol.RemotePermissionMode
import com.bitfun.mobile.core.protocol.RemoteModelCatalog
import com.bitfun.mobile.core.protocol.CommandStatusResponse
import com.bitfun.mobile.core.protocol.PermissionModeResponse
import com.bitfun.mobile.core.protocol.SetSessionModelResponse
import com.bitfun.mobile.core.protocol.SendMessageResponse
import com.bitfun.mobile.core.protocol.SessionItemResponse
import com.bitfun.mobile.core.protocol.SessionListResponse
import com.bitfun.mobile.core.protocol.WorkspaceInfoResponse
import com.bitfun.mobile.core.transport.PairedRoom
import com.bitfun.mobile.core.transport.RemoteCommandTransport
import com.bitfun.mobile.core.transport.send
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.put
import kotlin.time.Clock

/** The remote session feature: the session list plus the conversation that is open. */
public class RemoteSessionStore internal constructor(
    private val scope: CoroutineScope,
    private val transport: RemoteCommandTransport,
    private val deviceKey: String? = null,
    private val persistence: MobilePersistenceStores? = null,
) {
    private val persistenceEnabled: Boolean get() = persistence != null && !deviceKey.isNullOrBlank()
    private val _state = MutableStateFlow<RemoteSessionUiState>(RemoteSessionUiState.Idle)
    public val state: StateFlow<RemoteSessionUiState> = _state.asStateFlow()
    private val _connectionPhase = MutableStateFlow(ConnectionPhase.IDLE)
    public val connectionPhase: StateFlow<ConnectionPhase> = _connectionPhase.asStateFlow()
    private val timelineStore = ChatTimelineStore()
    private val controller = ChatSessionController.create(scope, RoomPoller(transport), ControllerCallbacks())
    private var work: Job? = null
    private var modelCatalog: RemoteModelCatalog? = null
    private var modelCatalogFailure: ModelCatalogFailure? = null
    private val locallyCreatedSessions: MutableMap<String, RemoteSession> = mutableMapOf()

    /**
     * The re-read of the transcript that follows a turn ending.
     *
     * Its own job rather than [work]: it must not cancel, or be cancelled by, the
     * list request a user action started, and the settle window fires the request
     * repeatedly — one in flight is enough.
     */
    private var turnEndSync: Job? = null

    /**
     * The workspace the desktop currently has open, learned from `get_workspace_info`.
     *
     * `list_sessions` and `create_session` are rejected outright when this is
     * missing, so it is resolved before either is sent rather than passed down
     * from the UI — the same shape as `RemoteSessionManager.workspace`.
     */
    private var workspacePath: String = ""

    public fun dispatch(intent: RemoteSessionIntent) {
        val current = _state.value as? RemoteSessionUiState.Ready
        when (intent) {
            RemoteSessionIntent.Load, RemoteSessionIntent.Refresh ->
                load(current?.query.orEmpty(), current?.agentFilter ?: SessionAgentFilter.ALL)
            RemoteSessionIntent.LoadMore -> loadMore()
            RemoteSessionIntent.LoadOlderMessages -> loadOlderMessages()
            is RemoteSessionIntent.Search ->
                load(intent.query, current?.agentFilter ?: SessionAgentFilter.ALL)
            is RemoteSessionIntent.SetAgentFilter -> load(current?.query.orEmpty(), intent.filter)
            is RemoteSessionIntent.Open -> open(intent.sessionId)
            is RemoteSessionIntent.CreateSession -> createSession(intent)
            is RemoteSessionIntent.DeleteSession -> deleteSession(intent.sessionId)
            is RemoteSessionIntent.RenameSession -> renameSession(intent)
            is RemoteSessionIntent.AnswerQuestion -> runAction(
                intent.sessionId,
                RemoteCommand(
                    cmd = "answer_question",
                    toolId = intent.toolId,
                    // The desktop forwards this opaquely to the tool. Both spellings
                    // are what `RemoteQuestionAnswerPayload` sends from HarmonyOS.
                    answers = buildJsonObject {
                        put("answer", intent.answer)
                        put("0", intent.answer)
                    },
                ),
            )
            is RemoteSessionIntent.AnswerStructuredQuestion -> runAction(
                intent.sessionId,
                RemoteCommand(
                    cmd = "answer_question",
                    toolId = intent.toolId,
                    answers = buildJsonObject {
                        intent.answers.forEach { answer ->
                            put(
                                answer.index.toString(),
                                when (val value = answer.value) {
                                    is QuestionAnswerValue.Text -> JsonPrimitive(value.text)
                                    is QuestionAnswerValue.Choice -> JsonArray(value.values.map(::JsonPrimitive))
                                },
                            )
                        }
                    },
                ),
            )
            is RemoteSessionIntent.UpdateDraft -> updateDraft(intent.text)
            is RemoteSessionIntent.SendMessage -> sendMessage(intent)
            is RemoteSessionIntent.CancelTurn -> cancelTurn(intent)
            is RemoteSessionIntent.ApproveTool -> approveTool(intent)
            is RemoteSessionIntent.RejectTool -> runAction(
                intent.sessionId,
                RemoteCommand(cmd = "reject_tool", toolId = intent.toolId, reason = intent.reason),
            )
            is RemoteSessionIntent.CancelTool -> runAction(
                intent.sessionId,
                RemoteCommand(cmd = "cancel_tool", toolId = intent.toolId, reason = intent.reason),
            )
            is RemoteSessionIntent.SetPermissionMode -> setPermissionMode(intent)
            is RemoteSessionIntent.RefreshPermissionMode -> refreshPermissionMode()
            RemoteSessionIntent.RefreshModelCatalog -> refreshModelCatalog()
            is RemoteSessionIntent.SelectModel -> selectModel(intent)
            RemoteSessionIntent.Stop -> stop()
        }
    }

    public fun stop() {
        work?.cancel()
        work = null
        turnEndSync?.cancel()
        turnEndSync = null
        controller.stop()
        _connectionPhase.value = ConnectionPhase.DISCONNECTED
    }

    private fun load(query: String, filter: SessionAgentFilter) {
        if (_state.value is RemoteSessionUiState.Loading) return
        val current = _state.value as? RemoteSessionUiState.Ready
        if (current == null && persistenceEnabled) {
            val cached = persistence!!.remoteSessions.load(deviceKey!!)
            if (cached.isNotEmpty()) {
                _state.value = RemoteSessionUiState.Ready(
                    sessions = cached.map(::toRemoteSession), selectedSessionId = null, timeline = null,
                    busy = true, permissionMode = null, permissionModeFailure = null,
                    query = query, agentFilter = filter,
                    hasMore = persistence.remoteSessions.hasMore(deviceKey), hasMoreMessages = false,
                    modelCatalog = null,
                )
            }
        }
        if (current == null) _connectionPhase.value = ConnectionPhase.CONNECTING
        work?.cancel()
        // Searching or switching tabs keeps the list on screen; only a cold start
        // blanks it, so typing in the search box does not flash a spinner.
        _state.value = (_state.value as? RemoteSessionUiState.Ready)?.copy(busy = true, query = query, agentFilter = filter)
            ?: RemoteSessionUiState.Loading
        work = scope.launch {
            try {
                if (!resolveWorkspacePath()) {
                    failKnown(RemoteSessionFailureReason.NO_WORKSPACE, current)
                    return@launch
                }
                val page = listSessions(0, query, filter)
                val catalog = loadModelCatalog(force = false)
                if (persistenceEnabled && query.isEmpty() && filter == SessionAgentFilter.ALL) {
                    persistence!!.remoteSessions.save(deviceKey!!, page.sessions.map(::toPersistedSession), page.hasMore)
                }
                _state.value = RemoteSessionUiState.Ready(
                    sessions = page.sessions,
                    selectedSessionId = current?.selectedSessionId,
                    timeline = currentTimeline(),
                    busy = false,
                    permissionMode = current?.permissionMode,
                    permissionModeFailure = current?.permissionModeFailure,
                    query = query,
                    agentFilter = filter,
                    hasMore = page.hasMore,
                    hasMoreMessages = current?.hasMoreMessages ?: false,
                    modelCatalog = catalog.catalog ?: current?.modelCatalog,
                    modelCatalogFailure = catalog.failure,
                    draft = current?.draft ?: "",
                )
                markConnected()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                handleFailure(error, _state.value as? RemoteSessionUiState.Ready)
            }
        }
    }

    private fun loadMore() {
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        if (!current.hasMore || current.busy) return
        setBusy(current, true)
        work?.cancel()
        work = scope.launch {
            try {
                val page = listSessions(current.sessions.size, current.query, current.agentFilter)
                val known = current.sessions.mapTo(mutableSetOf()) { it.id }
                val ready = (_state.value as? RemoteSessionUiState.Ready) ?: current
                val sessions = current.sessions + page.sessions.filterNot { it.id in known }
                if (persistenceEnabled && current.query.isEmpty() && current.agentFilter == SessionAgentFilter.ALL) {
                    persistence!!.remoteSessions.save(deviceKey!!, sessions.map(::toPersistedSession), page.hasMore)
                }
                _state.value = ready.copy(
                    sessions = sessions,
                    hasMore = page.hasMore,
                    busy = false,
                )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Throwable) {
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
            }
        }
    }

    /**
     * Reads the desktop's open workspace, returning false when it has none.
     *
     * Checking here rather than letting `list_sessions` fail keeps the reason a
     * typed one: the desktop answers a missing workspace with a prose message
     * that the phone would otherwise have to pattern-match.
     */
    private suspend fun resolveWorkspacePath(): Boolean {
        val info = transport.send<WorkspaceInfoResponse>(RemoteCommand(cmd = "get_workspace_info"))
        workspacePath = (info.path ?: info.workspacePath).orEmpty().trim()
        return workspacePath.isNotEmpty() && workspacePath != "/"
    }

    private suspend fun listSessions(offset: Int, query: String, filter: SessionAgentFilter): SessionPage {
        val trimmedQuery = query.trim()
        // `list_sessions` cannot apply either the mobile ACP visibility rule or
        // the agent tab. Pull from the start until there are enough visible rows
        // so an invisible server row never creates a short page or a dishonest
        // `hasMore` result.
        val targetCount = offset + PAGE_SIZE
        val filtered = mutableListOf<RemoteSession>()
        var pageOffset = 0
        var hasMore = true
        val pageSize = if (filter == SessionAgentFilter.ALL) PAGE_SIZE else FILTER_PAGE_SIZE
        while (hasMore && filtered.size < targetCount) {
            val response = sendListSessions(pageSize, pageOffset, trimmedQuery)
            val sessions = response.sessions.map(RemoteResponseMapper::session)
            sessions.filterTo(filtered) {
                SessionAgentTypes.isMobileVisible(it.agentType) && filter.matches(it.agentType)
            }
            hasMore = response.hasMore
            pageOffset += sessions.size
            if (sessions.isEmpty()) break
        }
        val serverIds = filtered.mapTo(mutableSetOf()) { it.id }
        serverIds.forEach(locallyCreatedSessions::remove)
        val projected = mergeLocallyCreated(filtered, trimmedQuery, filter)
        return SessionPage(
            sessions = projected.subList(minOf(offset, projected.size), minOf(targetCount, projected.size)).toList(),
            hasMore = projected.size > targetCount || hasMore,
        )
    }

    private fun mergeLocallyCreated(
        sessions: List<RemoteSession>,
        query: String,
        filter: SessionAgentFilter,
    ): List<RemoteSession> {
        val known = sessions.mapTo(mutableSetOf()) { it.id }
        val local = locallyCreatedSessions.values.filter { session ->
            session.id !in known &&
                SessionAgentTypes.isMobileVisible(session.agentType) &&
                filter.matches(session.agentType) &&
                (query.isEmpty() || session.title.contains(query, ignoreCase = true) ||
                    session.workspaceName.orEmpty().contains(query, ignoreCase = true) ||
                    session.workspacePath.orEmpty().contains(query, ignoreCase = true))
        }
        return local + sessions
    }

    /**
     * A model catalog is useful before a session exists. Failure is deliberately
     * non-fatal: the create screen hides the picker and every other remote
     * feature remains available.
     *
     * Every catalog failure is typed as [ModelCatalogFailure.LOAD_FAILED] until
     * the transport exposes a distinguishing peer-capability signal. A generic
     * rejection or malformed response is not proof of an old peer: a modern
     * desktop can reject the command transiently, and a malformed response can be
     * a local protocol fault.
     *
     * [force] re-requests even when a catalog is already cached, which is how a
     * settings retry reaches a catalog that a transient failure lost.
     */
    private suspend fun loadModelCatalog(force: Boolean): ModelCatalogLoadResult {
        if (!force) {
            modelCatalog?.let { return ModelCatalogLoadResult(it, null) }
        }
        return try {
            val catalog = transport.send<ModelCatalogResponse>(RemoteCommand(cmd = "get_model_catalog")).catalog
                ?.takeUnless { it.version == 0L && it.models.isEmpty() }
                ?.also { modelCatalog = it }
            modelCatalogFailure = null
            ModelCatalogLoadResult(catalog, null)
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Throwable) {
            modelCatalogFailure = ModelCatalogFailure.LOAD_FAILED
            ModelCatalogLoadResult(null, ModelCatalogFailure.LOAD_FAILED)
        }
    }

    private data class ModelCatalogLoadResult(
        val catalog: RemoteModelCatalog?,
        val failure: ModelCatalogFailure?,
    )

    private suspend fun sendListSessions(limit: Int, offset: Int, query: String): SessionListResponse =
        transport.send(
            RemoteCommand(
                cmd = "list_sessions",
                workspacePath = workspacePath,
                limit = limit,
                offset = offset,
                query = query.takeIf(String::isNotEmpty),
            ),
        )

    private fun open(sessionId: String) {
        val normalized = sessionId.trim()
        if (normalized.isEmpty()) {
            _state.value = RemoteSessionUiState.Failed(RemoteSessionFailureReason.SESSION_NOT_FOUND)
            _connectionPhase.value = ConnectionPhase.FAILED
            return
        }
        val current = _state.value as? RemoteSessionUiState.Ready
        val restoredDraft = if (persistenceEnabled) persistence!!.drafts.load(draftId(normalized)).orEmpty() else ""
        if (persistenceEnabled) {
            val cached = persistence!!.remoteTranscripts.load(deviceKey!!, normalized)
            if (cached.isNotEmpty()) {
                timelineStore.reset(normalized)
                timelineStore.setPersistedMessages(cached.map(::toChatMessage))
                persistence.remoteTranscripts.loadCursor(deviceKey, normalized)?.let { cursor ->
                    timelineStore.setCursor(ChatSessionCursor(
                        cursor.pollVersion.toIntOrNull() ?: 0,
                        cursor.knownMessageCount,
                        cursor.knownModelCatalogVersion.toLongOrNull() ?: 0L,
                    ))
                }
                _state.value = RemoteSessionUiState.Ready(
                    sessions = current?.sessions.orEmpty(), selectedSessionId = normalized,
                    timeline = timelineStore.snapshot(), busy = true,
                    permissionMode = current?.permissionMode, permissionModeFailure = current?.permissionModeFailure,
                    query = current?.query.orEmpty(), agentFilter = current?.agentFilter ?: SessionAgentFilter.ALL,
                    hasMore = current?.hasMore ?: false, hasMoreMessages = false,
                    modelCatalog = modelCatalog ?: current?.modelCatalog,
                    modelCatalogFailure = modelCatalogFailure ?: current?.modelCatalogFailure,
                    draft = restoredDraft,
                )
            }
        }
        if (current == null) _connectionPhase.value = ConnectionPhase.CONNECTING
        work?.cancel()
        _state.value = (_state.value as? RemoteSessionUiState.Ready)?.copy(busy = true) ?: current?.copy(busy = true)
            ?: RemoteSessionUiState.Loading
        work = scope.launch {
            try {
                val opened = openSession(normalized)
                _state.value = RemoteSessionUiState.Ready(
                    sessions = current?.sessions.orEmpty(),
                    selectedSessionId = normalized,
                    timeline = timelineStore.snapshot(),
                    busy = false,
                    permissionMode = opened.permission.mode,
                    permissionModeFailure = opened.permission.failure,
                    query = current?.query.orEmpty(),
                    agentFilter = current?.agentFilter ?: SessionAgentFilter.ALL,
                    hasMore = current?.hasMore ?: false,
                    hasMoreMessages = opened.hasMoreMessages,
                    modelCatalog = modelCatalog ?: current?.modelCatalog,
                    modelCatalogFailure = modelCatalogFailure ?: current?.modelCatalogFailure,
                    draft = restoredDraft,
                )
                markConnected()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                handleFailure(error, _state.value as? RemoteSessionUiState.Ready)
            }
        }
    }

    /** Loads a session's history, starts polling it, and reports its permission mode. */
    private suspend fun openSession(sessionId: String): OpenedSession {
        val response = transport.send<com.bitfun.mobile.core.protocol.SessionMessagesResponse>(
            RemoteCommand(cmd = "get_session_messages", sessionId = sessionId, limit = 100),
        )
        val cursor = timelineStore.snapshot().cursor.takeIf { timelineStore.snapshot().sessionId == sessionId }
        timelineStore.reset(sessionId)
        timelineStore.setPersistedMessages(response.messages.map(RemoteResponseMapper::chatMessage))
        controller.start(
            sessionId,
            ChatSessionCursor(
                pollVersion = 0,
                knownMessageCount = response.messages.size,
                knownModelCatalogVersion = cursor?.knownModelCatalogVersion ?: 0L,
            ),
        )
        persistTranscript(sessionId)
        return OpenedSession(readPermissionMode(), response.hasMore)
    }

    private data class OpenedSession(
        val permission: OpenedPermission,
        val hasMoreMessages: Boolean,
    )

    private fun loadOlderMessages() {
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        val sessionId = current.selectedSessionId.orEmpty()
        val beforeMessageId = current.timeline?.persistedMessages?.firstOrNull()?.id.orEmpty()
        if (sessionId.isEmpty() || beforeMessageId.isEmpty() || !current.hasMoreMessages || current.busy) return
        setBusy(current, true)
        work?.cancel()
        work = scope.launch {
            try {
                val response = transport.send<com.bitfun.mobile.core.protocol.SessionMessagesResponse>(
                    RemoteCommand(
                        cmd = "get_session_messages",
                        sessionId = sessionId,
                        limit = 100,
                        beforeMessageId = beforeMessageId,
                    ),
                )
                if (timelineStore.snapshot().sessionId != sessionId) return@launch
                val visible = timelineStore.snapshot().persistedMessages
                val visibleIds = visible.mapTo(mutableSetOf()) { it.id }
                val older = response.messages
                    .map(RemoteResponseMapper::chatMessage)
                    .filterNot { it.id in visibleIds }
                timelineStore.setPersistedMessages(older + visible)
                persistTranscript(sessionId)
                val ready = (_state.value as? RemoteSessionUiState.Ready) ?: current
                _state.value = ready.copy(
                    timeline = timelineStore.snapshot(),
                    busy = false,
                    hasMoreMessages = response.hasMore,
                )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Throwable) {
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
            }
        }
    }

    /**
     * The permission mode alone, with its own failure.
     *
     * Unaddressed, as `RemoteCommandFactory.getPermissionMode()` sends it: the
     * desktop holds one mode for all of its sessions, so naming a session here
     * would ask the wrong question and get the same answer.
     *
     * Deliberately not allowed to throw: the transcript above it has already
     * loaded, and losing a whole session because one settings read failed is a
     * far worse outcome than a settings section that says so and offers Refresh.
     */
    private suspend fun readPermissionMode(): OpenedPermission = try {
        OpenedPermission(
            transport.send<PermissionModeResponse>(
                RemoteCommand(cmd = "get_permission_mode"),
            ).mode?.toUiMode() ?: SessionPermissionMode.UNKNOWN,
            null,
        )
    } catch (cancelled: CancellationException) {
        throw cancelled
    } catch (error: Throwable) {
        OpenedPermission(null, PermissionModeFailure.LOAD)
    }

    private data class OpenedPermission(
        val mode: SessionPermissionMode?,
        val failure: PermissionModeFailure?,
    )

    private fun createSession(intent: RemoteSessionIntent.CreateSession) {
        val current = _state.value as? RemoteSessionUiState.Ready
        if (current == null) _connectionPhase.value = ConnectionPhase.CONNECTING
        work?.cancel()
        _state.value = current?.copy(busy = true) ?: RemoteSessionUiState.Loading
        work = scope.launch {
            try {
                val requestedWorkspacePath = intent.workspacePath?.trim().orEmpty()
                // An explicit path is the cross-workspace sidebar flow: creating
                // there must not change the desktop's active workspace. The
                // ordinary create flow still re-reads the active path so a
                // recent workspace selection cannot race a cached value.
                if (requestedWorkspacePath.isEmpty() && !resolveWorkspacePath()) {
                    failKnown(RemoteSessionFailureReason.NO_WORKSPACE, current)
                    return@launch
                }
                val targetWorkspacePath = requestedWorkspacePath.ifEmpty { workspacePath }
                val created = transport.send<CreateSessionResponse>(
                    RemoteCommand(
                        cmd = "create_session",
                        agentType = intent.agentType,
                        sessionName = SessionNaming.wireSessionName(intent.agentType, intent.title),
                        workspacePath = targetWorkspacePath,
                    ),
                )
                val sessionId = created.resolvedSessionId?.trim().orEmpty()
                if (sessionId.isEmpty()) {
                    failKnown(RemoteSessionFailureReason.PROTOCOL_MISMATCH, current)
                    return@launch
                }
                intent.modelId?.trim()?.takeIf(String::isNotEmpty)?.let { modelId ->
                    transport.send<SetSessionModelResponse>(
                        RemoteCommand(cmd = "set_session_model", sessionId = sessionId, modelId = modelId),
                    )
                }
                val opened = openSession(sessionId)
                intent.instruction.trim().takeIf(String::isNotEmpty)?.let { instruction ->
                    val sent = transport.send<SendMessageResponse>(
                        RemoteCommand(
                            cmd = "send_message",
                            sessionId = sessionId,
                            content = instruction,
                            agentType = intent.agentType,
                        ),
                    )
                    sent.turnId?.let(timelineStore::setLocalActiveTurn)
                    controller.nudge()
                }
                val now = Clock.System.now().toString()
                locallyCreatedSessions[sessionId] = RemoteSession(
                    id = sessionId,
                    title = created.title?.takeIf(String::isNotBlank)
                        ?: SessionNaming.fallbackTitle(intent.agentType),
                    agentType = intent.agentType,
                    status = "active",
                    updatedAt = now,
                    createdAt = now,
                    messageCount = if (intent.instruction.isBlank()) 0 else 1,
                    workspacePath = targetWorkspacePath,
                    workspaceName = null,
                )
                val page = listSessions(0, current?.query.orEmpty(), current?.agentFilter ?: SessionAgentFilter.ALL)
                _state.value = RemoteSessionUiState.Ready(
                    sessions = page.sessions,
                    selectedSessionId = sessionId,
                    timeline = timelineStore.snapshot(),
                    busy = false,
                    permissionMode = opened.permission.mode,
                    permissionModeFailure = opened.permission.failure,
                    query = current?.query.orEmpty(),
                    agentFilter = current?.agentFilter ?: SessionAgentFilter.ALL,
                    hasMore = page.hasMore,
                    hasMoreMessages = opened.hasMoreMessages,
                    modelCatalog = modelCatalog ?: current?.modelCatalog,
                    modelCatalogFailure = modelCatalogFailure ?: current?.modelCatalogFailure,
                    draft = "",
                )
                markConnected()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                handleFailure(error, current)
            }
        }
    }

    private fun deleteSession(sessionId: String) {
        val normalized = sessionId.trim()
        if (normalized.isEmpty()) return
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        setBusy(current, true)
        work?.cancel()
        work = scope.launch {
            try {
                transport.send<CommandStatusResponse>(
                    RemoteCommand(cmd = "delete_session", sessionId = normalized),
                )
                locallyCreatedSessions.remove(normalized)
                val closingOpenSession = current.selectedSessionId == normalized
                if (closingOpenSession) {
                    if (persistenceEnabled) persistence!!.drafts.delete(draftId(normalized))
                    controller.stop()
                    timelineStore.reset("")
                }
                val ready = (_state.value as? RemoteSessionUiState.Ready) ?: current
                _state.value = ready.copy(
                    sessions = current.sessions.filterNot { it.id == normalized },
                    selectedSessionId = current.selectedSessionId.takeUnless { closingOpenSession },
                    timeline = if (closingOpenSession) null else ready.timeline,
                    permissionMode = if (closingOpenSession) null else ready.permissionMode,
                    busy = false,
                )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
                handleFailure(error, current)
            }
        }
    }

    private fun renameSession(intent: RemoteSessionIntent.RenameSession) {
        val sessionId = intent.sessionId.trim()
        val title = intent.title.trim()
        if (sessionId.isEmpty() || title.isEmpty()) return
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        if (current.sessions.any { it.id == sessionId && it.title == title }) return
        setBusy(current, true)
        work?.cancel()
        work = scope.launch {
            try {
                transport.send<CommandStatusResponse>(
                    RemoteCommand(cmd = "update_session_title", sessionId = sessionId, title = title),
                )
                val ready = (_state.value as? RemoteSessionUiState.Ready) ?: current
                _state.value = ready.copy(
                    sessions = ready.sessions.map { if (it.id == sessionId) it.copy(title = title) else it },
                    busy = false,
                )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
                handleFailure(error, current)
            }
        }
    }

    private class SessionPage(val sessions: List<RemoteSession>, val hasMore: Boolean)

    private fun currentTimeline() = timelineStore.snapshot().takeIf { it.sessionId.isNotEmpty() }

    private fun updateTimeline(snapshot: ChatSessionSnapshot) {
        if (snapshot.sessionId != timelineStore.snapshot().sessionId) return
        timelineStore.applySnapshot(snapshot)
        snapshot.modelCatalog?.let { catalog ->
            if (catalog.version > 0L || catalog.models.isNotEmpty()) modelCatalog = catalog
        }
        val current = _state.value
        if (current is RemoteSessionUiState.Ready) {
            _state.value = current.copy(
                timeline = timelineStore.snapshot(),
                modelCatalog = modelCatalog ?: current.modelCatalog,
            )
            markConnected()
        }
        if (snapshot.shouldSyncAfterTurnEnded) {
            persistTranscript(snapshot.sessionId)
            syncAfterTurnEnded(snapshot.sessionId)
        }
    }

    /**
     * Re-reads the transcript once the agent has stopped talking, as
     * `syncAfterRemoteTurnEnded` does.
     *
     * The last thing a poll reports is a turn whose status is no longer active,
     * and the store holds that finished turn on screen until the same text
     * arrives as a stored message — which the poll never sends, because from its
     * side nothing changed after the turn ended. Without this the transcript is
     * right but the composer keeps offering Stop for a turn that is over.
     */
    private fun syncAfterTurnEnded(sessionId: String) {
        if (sessionId.isEmpty() || turnEndSync?.isActive == true) return
        turnEndSync = scope.launch {
            try {
                val response = transport.send<com.bitfun.mobile.core.protocol.SessionMessagesResponse>(
                    RemoteCommand(cmd = "get_session_messages", sessionId = sessionId, limit = 100),
                )
                if (timelineStore.snapshot().sessionId != sessionId) return@launch
                timelineStore.setPersistedMessages(response.messages.map(RemoteResponseMapper::chatMessage))
                // Back to version zero, matching `onMessageCountKnown(0, …)`: the
                // transcript just came from the source of truth, so the next poll
                // should describe everything it knows rather than the delta since
                // a version whose messages have already been replaced.
                val cursor = ChatSessionCursor(
                    pollVersion = 0,
                    knownMessageCount = response.messages.size,
                    knownModelCatalogVersion = timelineStore.snapshot().cursor.knownModelCatalogVersion,
                )
                timelineStore.setCursor(cursor)
                persistTranscript(sessionId)
                controller.updateCursor(cursor)
                val current = _state.value
                if (current is RemoteSessionUiState.Ready) {
                    _state.value = current.copy(
                        timeline = timelineStore.snapshot(),
                        hasMoreMessages = response.hasMore,
                    )
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                // The transcript on screen is still the one the poll built, and
                // the poll is still running. Failing the session over a refresh
                // that only tidies up would lose more than it reports.
            }
        }
    }

    private fun sendMessage(intent: RemoteSessionIntent.SendMessage) {
        val sessionId = intent.sessionId.trim()
        val content = intent.content
        if (sessionId.isEmpty() || content.trim().isEmpty()) return
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        val wireImages = intent.images?.map { image ->
            com.bitfun.mobile.core.protocol.ImageAttachment(
                name = image.id,
                dataUrl = image.dataUrl,
            )
        }
        val imageContexts = intent.images?.map { image ->
            com.bitfun.mobile.core.protocol.RemoteImageContext(
                id = image.id,
                imagePath = null,
                dataUrl = image.dataUrl,
                mimeType = image.mimeType,
                metadata = null,
            )
        }
        val local = ChatMessage(
            id = "msg-${Clock.System.now().toEpochMilliseconds()}",
            role = "user",
            text = content,
            status = "sent",
            renderVersion = null,
            turnId = null,
            detail = null,
            timestamp = null,
            thinking = null,
            tools = null,
            items = null,
            images = wireImages,
        )
        timelineStore.appendOptimisticMessage(local)
        setBusy(current, true)
        work?.cancel()
        work = scope.launch {
            try {
                val agentType = current.sessions.firstOrNull { it.id == sessionId }?.agentType
                    ?: locallyCreatedSessions[sessionId]?.agentType
                val response = transport.send<SendMessageResponse>(
                    RemoteCommand(
                        cmd = "send_message",
                        sessionId = sessionId,
                        content = content,
                        agentType = agentType,
                        imageContexts = imageContexts,
                    ),
                )
                response.turnId?.let(timelineStore::setLocalActiveTurn)
                controller.nudge()
                if (persistenceEnabled) persistence!!.drafts.delete(draftId(sessionId))
                val ready = ((_state.value as? RemoteSessionUiState.Ready) ?: current)
                if (ready.selectedSessionId == sessionId) _state.value = ready.copy(draft = "")
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                timelineStore.markOptimisticMessageFailed(local.id)
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
                handleFailure(error, current)
            }
        }
    }

    private fun updateDraft(text: String) {
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        val id = current.selectedSessionId ?: return
        if (persistenceEnabled) {
            if (text.isEmpty()) persistence!!.drafts.delete(draftId(id))
            else persistence!!.drafts.save(draftId(id), text)
        }
        _state.value = current.copy(draft = text)
    }

    private fun draftId(sessionId: String): String = "remote-composer:$deviceKey:$sessionId"

    private fun cancelTurn(intent: RemoteSessionIntent.CancelTurn) {
        val sessionId = intent.sessionId.trim()
        if (sessionId.isEmpty()) return
        runAction(sessionId, RemoteCommand(cmd = "cancel_task", sessionId = sessionId, turnId = intent.turnId))
    }

    private fun approveTool(intent: RemoteSessionIntent.ApproveTool) {
        if (intent.updatedInput != null) {
            // The desktop confirm_tool handler accepts only tool_id. Sending an
            // edited approval would approve while silently dropping the edit, so
            // gate it to the typed unsupported fact (ToolApprovalEditContract).
            return
        }
        runAction(intent.sessionId, RemoteCommand(cmd = "confirm_tool", toolId = intent.toolId))
    }

    private fun setPermissionMode(intent: RemoteSessionIntent.SetPermissionMode) {
        val wireMode = intent.mode.toWireMode() ?: return
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        setBusy(current, true)
        work?.cancel()
        work = scope.launch {
            try {
                transport.send<CommandStatusResponse>(
                    RemoteCommand(cmd = "set_permission_mode", mode = wireMode),
                )
                val ready = (_state.value as? RemoteSessionUiState.Ready) ?: current
                _state.value = ready.copy(
                    busy = false,
                    permissionMode = intent.mode,
                    permissionModeFailure = null,
                )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                // The session itself is fine — only this one setting failed, so
                // the failure stays inside the permission section rather than
                // replacing the transcript the user is reading.
                val ready = (_state.value as? RemoteSessionUiState.Ready) ?: current
                _state.value = ready.copy(
                    busy = false,
                    permissionModeFailure = PermissionModeFailure.SAVE,
                )
            }
        }
    }

    private fun refreshPermissionMode() {
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        setBusy(current, true)
        work?.cancel()
        work = scope.launch {
            val permission = readPermissionMode()
            val ready = (_state.value as? RemoteSessionUiState.Ready) ?: current
            _state.value = ready.copy(
                busy = false,
                permissionMode = permission.mode ?: ready.permissionMode,
                permissionModeFailure = permission.failure,
            )
        }
    }

    /**
     * Re-reads the model catalog alone, the way [refreshPermissionMode] re-reads
     * the permission mode: the session list, transcript, and draft stay on
     * screen, and only the model section changes. A failure keeps the store in
     * [RemoteSessionUiState.Ready] with the typed failure instead of taking the
     * transcript down with it.
     */
    private fun refreshModelCatalog() {
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        if (current.busy) return
        // Forward-compat: a future transport may produce a real unsupported-by-
        // peer signal. That is the only case where a retry cannot help, because
        // every generic failure is typed as LOAD_FAILED and remains retryable.
        if (modelCatalogFailure == ModelCatalogFailure.UNSUPPORTED_BY_PEER) return
        setBusy(current, true)
        work?.cancel()
        work = scope.launch {
            try {
                val result = loadModelCatalog(force = true)
                val timeline = result.catalog?.let { catalog ->
                    val snapshot = timelineStore.snapshot()
                    if (snapshot.sessionId.isNotEmpty()) {
                        timelineStore.setModelCatalog(catalog, snapshot.selectedModelId)
                        timelineStore.snapshot()
                    } else {
                        null
                    }
                }
                val ready = (_state.value as? RemoteSessionUiState.Ready) ?: current
                _state.value = ready.copy(
                    busy = false,
                    modelCatalog = result.catalog ?: ready.modelCatalog,
                    modelCatalogFailure = result.failure,
                    timeline = timeline ?: ready.timeline,
                )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Throwable) {
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
            }
        }
    }

    private fun selectModel(intent: RemoteSessionIntent.SelectModel) {
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        if (intent.modelId.trim().isEmpty()) return
        setBusy(current, true)
        work?.cancel()
        work = scope.launch {
            try {
                val response = transport.send<SetSessionModelResponse>(
                    RemoteCommand(cmd = "set_session_model", sessionId = intent.sessionId, modelId = intent.modelId),
                )
                timelineStore.setSelectedModelId(response.modelId ?: intent.modelId)
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
                handleFailure(error, current)
            }
        }
    }

    private fun runAction(sessionId: String, command: RemoteCommand) {
        val current = _state.value as? RemoteSessionUiState.Ready ?: return
        setBusy(current, true)
        work?.cancel()
        work = scope.launch {
            try {
                transport.send<CommandStatusResponse>(command.copy(sessionId = command.sessionId ?: sessionId))
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
                controller.nudge()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                setBusy((_state.value as? RemoteSessionUiState.Ready) ?: current, false)
                handleFailure(error, current)
            }
        }
    }

    private fun setBusy(
        current: RemoteSessionUiState.Ready,
        busy: Boolean,
        permissionMode: SessionPermissionMode? = current.permissionMode,
    ) {
        _state.value = current.copy(busy = busy, permissionMode = permissionMode)
    }

    private fun SessionPermissionMode.toWireMode(): RemotePermissionMode? = when (this) {
        SessionPermissionMode.ASK -> RemotePermissionMode.Ask
        SessionPermissionMode.AUTO -> RemotePermissionMode.Auto
        SessionPermissionMode.FULL_ACCESS -> RemotePermissionMode.FullAccess
        SessionPermissionMode.UNKNOWN -> null
    }

    private fun RemotePermissionMode.toUiMode(): SessionPermissionMode = when (this) {
        RemotePermissionMode.Ask -> SessionPermissionMode.ASK
        RemotePermissionMode.Auto -> SessionPermissionMode.AUTO
        RemotePermissionMode.FullAccess -> SessionPermissionMode.FULL_ACCESS
        RemotePermissionMode.Unknown -> SessionPermissionMode.UNKNOWN
    }

    private fun persistTranscript(sessionId: String) {
        if (!persistenceEnabled || sessionId.isEmpty()) return
        val snapshot = timelineStore.snapshot()
        if (snapshot.sessionId != sessionId) return
        val p = persistence!!
        val window = snapshot.persistedMessages.map { toPersisted(sessionId, it) }
        val windowIds = window.mapTo(mutableSetOf()) { it.messageId }
        // A paginated re-read (limit 100) must not truncate pages the user already
        // loaded: keep older cached rows the current window does not cover.
        val older = p.remoteTranscripts.load(deviceKey!!, sessionId).filterNot { it.messageId in windowIds }
        p.remoteTranscripts.replace(deviceKey, sessionId, older + window)
        p.remoteTranscripts.saveCursor(deviceKey, sessionId, PersistedRemoteCursor(
            pollVersion = snapshot.cursor.pollVersion.toString(),
            knownMessageCount = snapshot.cursor.knownMessageCount,
            knownModelCatalogVersion = snapshot.cursor.knownModelCatalogVersion.toString(),
        ))
    }

    private fun toRemoteSession(s: PersistedRemoteSession): RemoteSession = RemoteSession(
        id = s.sessionId, title = s.title, agentType = s.agentType, status = s.status,
        updatedAt = s.updatedAt, createdAt = s.createdAt, messageCount = s.messageCount,
        workspacePath = s.workspacePath, workspaceName = s.workspaceName,
    )

    private fun toPersistedSession(s: RemoteSession): PersistedRemoteSession = PersistedRemoteSession(
        sessionId = s.id, title = s.title, agentType = s.agentType, status = s.status,
        updatedAt = s.updatedAt, createdAt = s.createdAt, messageCount = s.messageCount,
        lastMessageId = "", workspacePath = s.workspacePath, workspaceName = s.workspaceName,
    )

    private inner class ControllerCallbacks : ChatSessionControllerCallbacks {
        override fun onSnapshot(snapshot: ChatSessionSnapshot) {
            updateTimeline(snapshot)
        }

        override fun onError(error: Throwable) {
            handleFailure(error, _state.value as? RemoteSessionUiState.Ready)
        }

        override fun canPoll(sessionId: String): Boolean =
            _state.value is RemoteSessionUiState.Ready && timelineStore.snapshot().sessionId == sessionId
    }

    /**
     * A dropped transport must not replace an already-rendered transcript with a
     * list error. The poll loop keeps running, so its next successful response is
     * also the recovery probe and [updateTimeline] moves the phase back to live.
     */
    private fun handleFailure(error: Throwable, current: RemoteSessionUiState.Ready?) {
        val failed = remoteSessionFailure(error)
        if (current == null) {
            _state.value = failed
            _connectionPhase.value = ConnectionPhase.FAILED
            return
        }
        _state.value = current.copy(busy = false)
        _connectionPhase.value = when (failed.reason) {
            RemoteSessionFailureReason.NETWORK,
            RemoteSessionFailureReason.TIMEOUT,
            RemoteSessionFailureReason.TRANSPORT,
            -> ConnectionPhase.RECONNECTING

            else -> ConnectionPhase.FAILED
        }
    }

    private fun failKnown(reason: RemoteSessionFailureReason, current: RemoteSessionUiState.Ready?) {
        if (current == null) {
            _state.value = RemoteSessionUiState.Failed(reason)
        } else {
            _state.value = current.copy(busy = false)
        }
        _connectionPhase.value = ConnectionPhase.FAILED
    }

    private fun markConnected() {
        _connectionPhase.value = ConnectionPhase.CONNECTED
    }

    private class RoomPoller(private val transport: RemoteCommandTransport) : ChatSessionPoller {
        override suspend fun pollSession(
            sessionId: String,
            sinceVersion: Int,
            knownMessageCount: Int,
            knownModelCatalogVersion: Long,
        ): PollSessionResult {
            val response = transport.send<PollSessionResponse>(
                RemoteCommand(
                    cmd = "poll_session",
                    sessionId = sessionId,
                    sinceVersion = sinceVersion,
                    knownMessageCount = knownMessageCount,
                    knownModelCatalogVersion = knownModelCatalogVersion,
                ),
            )
            val version = if (response.version > 0) response.version else sinceVersion
            return PollSessionResult(
                version = response.version,
                changed = response.changed,
                sessionState = response.sessionState.orEmpty(),
                title = response.title.orEmpty(),
                newMessages = response.newMessages.map(RemoteResponseMapper::chatMessage),
                totalMessageCount = response.totalMessageCount ?: knownMessageCount,
                activeTurn = response.activeTurn?.let { RemoteResponseMapper.activeTurn(it, version) },
                modelCatalog = response.modelCatalog,
            )
        }
    }

    public companion object {
        /** One screenful of sessions. Matches the desktop's own `list_sessions` default. */
        private const val PAGE_SIZE: Int = 30

        /**
         * Window used while narrowing by agent type. The desktop clamps `limit`
         * to 100, so asking for more only costs round trips.
         */
        private const val FILTER_PAGE_SIZE: Int = 100

        internal fun create(scope: CoroutineScope, room: PairedRoom): RemoteSessionStore =
            RemoteSessionStore(scope, room.transport)

        internal fun create(scope: CoroutineScope, transport: RemoteCommandTransport): RemoteSessionStore =
            RemoteSessionStore(scope, transport)

        internal fun create(
            scope: CoroutineScope,
            transport: RemoteCommandTransport,
            deviceKey: String?,
            persistence: MobilePersistenceStores?,
        ): RemoteSessionStore = RemoteSessionStore(scope, transport, deviceKey, persistence)

        internal fun create(
            scope: CoroutineScope,
            room: PairedRoom,
            deviceKey: String?,
            persistence: MobilePersistenceStores?,
        ): RemoteSessionStore = RemoteSessionStore(scope, room.transport, deviceKey, persistence)
    }
}

@Serializable
private data class StoredRemoteMessagePayload(
    val renderVersion: Int? = null,
    val turnId: String? = null,
    val detail: String? = null,
    val tools: List<RemoteToolStatusResponse>? = null,
    val items: List<ChatMessageItemResponse>? = null,
    val images: List<ImageAttachment>? = null,
)

private val STORE_JSON = Json { ignoreUnknownKeys = true }

private fun toPersisted(sessionId: String, m: ChatMessage): PersistedRemoteMessage = PersistedRemoteMessage(
    messageId = m.id, sessionId = sessionId, role = m.role, text = m.text, status = m.status,
    timestamp = m.timestamp, thinking = m.thinking,
    payloadJson = STORE_JSON.encodeToString(StoredRemoteMessagePayload(
        m.renderVersion, m.turnId, m.detail, m.tools, m.items, m.images,
    )),
)

private fun toChatMessage(m: PersistedRemoteMessage): ChatMessage {
    val payload = try {
        STORE_JSON.decodeFromString<StoredRemoteMessagePayload>(m.payloadJson)
    } catch (_: Throwable) {
        StoredRemoteMessagePayload()
    }
    return ChatMessage(
        id = m.messageId, role = m.role, text = m.text, status = m.status,
        renderVersion = payload.renderVersion, turnId = payload.turnId, detail = payload.detail,
        timestamp = m.timestamp, thinking = m.thinking, tools = payload.tools,
        items = payload.items, images = payload.images,
    )
}

internal object RemoteResponseMapper {
    fun session(item: SessionItemResponse): RemoteSession {
        val id = item.id.orEmpty()
        return RemoteSession(
            id = id,
            title = item.title?.takeIf(String::isNotEmpty) ?: "Session ${id.take(6)}",
            agentType = item.agentType ?: "code",
            status = item.status ?: "idle",
            updatedAt = item.updatedAt,
            createdAt = item.createdAt,
            messageCount = item.messageCount ?: 0,
            workspacePath = item.workspacePath,
            workspaceName = item.workspaceName,
        )
    }

    fun chatMessage(item: ChatMessageResponse): ChatMessage {
        val text = messageText(item.content, item.items)
        val tools = if (item.tools.isNotEmpty()) item.tools else itemTools(item.items)
        return ChatMessage(
            id = item.resolvedId ?: generatedId(item.role, item.timestamp.orEmpty(), text),
            role = item.role,
            text = text,
            status = if (item.role == "assistant") "done" else "sent",
            renderVersion = null,
            turnId = null,
            detail = messageDetail(tools),
            timestamp = item.timestamp,
            thinking = item.thinking,
            tools = tools,
            items = item.items,
            images = item.images,
        )
    }

    fun activeTurn(turn: ActiveTurnSnapshotResponse, renderVersion: Int = 0): ChatMessage {
        val tools = if (turn.tools.isNotEmpty()) turn.tools else itemTools(turn.items)
        return ChatMessage(
            id = "active-${turn.turnId}",
            role = "assistant",
            text = messageText(turn.text.orEmpty(), turn.items),
            status = turn.status,
            renderVersion = renderVersion,
            turnId = turn.turnId,
            detail = messageDetail(tools),
            timestamp = null,
            thinking = turn.thinking,
            tools = tools,
            items = turn.items,
            images = null,
        )
    }

    private fun messageText(content: String, items: List<ChatMessageItemResponse>): String =
        content.takeIf { it.trim().isNotEmpty() } ?: lastTopLevelText(items)

    private fun lastTopLevelText(items: List<ChatMessageItemResponse>): String = items.asReversed()
        .firstOrNull { item ->
            val type = item.type.orEmpty().lowercase()
            item.content.orEmpty().trim().isNotEmpty() && item.tool == null && item.isSubagent != true &&
                type !in setOf("thinking", "tool", "subagent", "agent")
        }?.content.orEmpty().trim()

    private fun itemTools(items: List<ChatMessageItemResponse>): List<com.bitfun.mobile.core.protocol.RemoteToolStatusResponse> =
        items.flatMap { item -> listOfNotNull(item.tool) + item.subItems.orEmpty().let(::itemTools) }

    private fun messageDetail(tools: List<com.bitfun.mobile.core.protocol.RemoteToolStatusResponse>): String =
        tools.joinToString("\n") { "${it.name ?: "Tool"} · ${it.status ?: "pending"}" }

    private fun generatedId(role: String, timestamp: String, text: String): String =
        "$role-$timestamp-${text.hashCode().toUInt().toString(36)}"
}
