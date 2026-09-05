package com.openbitfun.mobile.core.feature.generalchat

import com.openbitfun.mobile.core.domain.ChatMessage
import com.openbitfun.mobile.core.domain.ChatTimelineStore
import com.openbitfun.mobile.core.domain.ConversationEvent
import com.openbitfun.mobile.core.domain.GeneralChatExportFormatter
import com.openbitfun.mobile.core.feature.CloudSettingsSource
import com.openbitfun.mobile.core.feature.markdown.MarkdownParser
import com.openbitfun.mobile.core.feature.session.ComposerImage
import com.openbitfun.mobile.core.persistence.ChatLocalStore
import com.openbitfun.mobile.core.persistence.DraftStore
import com.openbitfun.mobile.core.persistence.PersistedChatMessage
import com.openbitfun.mobile.core.persistence.PersistedChatSession
import com.openbitfun.mobile.core.persistence.SecureStore
import com.openbitfun.mobile.core.transport.ModelProviderException
import com.openbitfun.mobile.core.transport.ModelProviderFailure
import com.openbitfun.mobile.core.transport.ModelProviderMessage
import com.openbitfun.mobile.core.transport.ModelProviderStreamClient
import com.openbitfun.mobile.core.transport.ModelProviderStreamResult
import com.openbitfun.mobile.core.protocol.ImageAttachment
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlin.time.Clock

internal data class GeneralChatStreamRequest(
    val baseUrl: String,
    val apiKey: String,
    val model: String,
    val messages: List<ModelProviderMessage>,
    /**
     * A reply cap, so the probe cannot be billed like a conversation: the
     * source asks for one token and one short word, and only needs to know
     * that something came back.
     */
    val maxTokens: Int,
    val systemPrompt: String,
)

internal fun interface GeneralChatStreamPort {
    suspend fun stream(
        request: GeneralChatStreamRequest,
        onDelta: (String) -> Unit,
    ): ModelProviderStreamResult
}

/** Shared General Chat orchestration; platform code only observes state and dispatches intents. */
public class GeneralChatStore internal constructor(
    private val scope: CoroutineScope,
    private val stream: GeneralChatStreamPort,
    private val drafts: DraftStore,
    private val chats: ChatLocalStore,
    secure: SecureStore,
) {
    private val config = GeneralChatConfigStore(secure)
    private val timeline = ChatTimelineStore()
    private val _state: MutableStateFlow<GeneralChatUiState>

    private var sessionId: String = ""
    private var request: Job? = null

    /**
     * Where the account's models come from, once something has signed in.
     *
     * Null until then, and null again on sign-out — this store is written so
     * that a source it does not have and a source that returns nothing look the
     * same from the outside.
     */
    private var cloudSettings: CloudSettingsSource? = null
    private var accountModelsJob: Job? = null

    /**
     * The probe runs on its own job so testing a new endpoint cannot cancel a
     * reply that is still streaming into the conversation behind the panel.
     */
    private var probe: Job? = null
    private var sequence: Long = 0

    init {
        val sessions = listSessions()
        sessionId = sessions.firstOrNull()?.id ?: newSessionId()
        timeline.reset(sessionId)
        timeline.setPersistedMessages(chats.loadMessages(sessionId).map(::restoreMessage))

        var stored = GeneralChatConfigUi("", "", false)
        var configured = false
        var configFailure: GeneralChatConfigFailure? = null
        var models = emptyList<GeneralChatModelUi>()
        var activeModelId = ""
        try {
            stored = config.snapshot()
            configured = config.activeEndpoint() != null
            models = config.catalog()
            activeModelId = config.activeModelId()
        } catch (_: Throwable) {
            // A platform keystore failure is not the same as an empty config.
            // Keep all provider data out of observable state and do not retry the
            // secure store while restoring the independent local conversation.
            stored = GeneralChatConfigUi("", "", false)
            configured = false
            configFailure = GeneralChatConfigFailure.SECURE_STORAGE
            models = emptyList()
            activeModelId = ""
        }

        _state = MutableStateFlow(
            GeneralChatUiState(
                configured = configured,
                config = stored,
                configFailure = configFailure,
                connectionTest = GeneralChatConnectionTestUi(),
                models = models,
                activeModelId = activeModelId,
                sessionId = sessionId,
                sessions = sessions,
                timeline = timeline.snapshot(),
                messages = projectMessages(),
                draft = drafts.load(draftId(sessionId)).orEmpty(),
                images = emptyList(),
                busy = false,
                failure = null,
                export = null,
            ),
        )
    }

    public val state: StateFlow<GeneralChatUiState> = _state.asStateFlow()

    public fun dispatch(intent: GeneralChatIntent) {
        when (intent) {
            is GeneralChatIntent.SaveConfig -> saveConfig(intent)
            GeneralChatIntent.ClearConfigFailure ->
                _state.value = _state.value.copy(configFailure = null)
            is GeneralChatIntent.TestConnection -> testConnection(intent)
            GeneralChatIntent.ClearConnectionTest ->
                _state.value = _state.value.copy(connectionTest = GeneralChatConnectionTestUi())
            is GeneralChatIntent.UpdateDraft -> updateDraft(intent.text)
            is GeneralChatIntent.SetImages -> setImages(intent.images)
            is GeneralChatIntent.SelectModel -> selectModel(intent.modelId)
            GeneralChatIntent.NewSession -> newSession()
            is GeneralChatIntent.SelectSession -> selectSession(intent.sessionId)
            is GeneralChatIntent.DeleteSession -> deleteSession(intent.sessionId)
            is GeneralChatIntent.RenameSession -> renameSession(intent.sessionId, intent.title)
            is GeneralChatIntent.ArchiveSession -> archiveSession(intent.sessionId, intent.archived)
            is GeneralChatIntent.PinSession -> pinSession(intent.sessionId, intent.pinned)
            is GeneralChatIntent.ExportSession -> exportSession(intent)
            GeneralChatIntent.ClearExport -> _state.value = _state.value.copy(export = null)
            GeneralChatIntent.Send -> send()
            GeneralChatIntent.Cancel -> cancel()
            GeneralChatIntent.ClearFailure -> clearFailure()
        }
    }

    public fun stop() {
        request?.cancel()
        request = null
        probe?.cancel()
        probe = null
        accountModelsJob?.cancel()
        accountModelsJob = null
    }

    /**
     * Points this store at the signed-in account, or at nothing.
     *
     * `loadGeneralChatAccountModels` is called on the same edge in the source:
     * once a cloud session exists, and again whenever it is replaced. Passing
     * null is sign-out, and drops the synced models immediately rather than
     * waiting for a fetch to fail — a signed-out phone must not keep answering
     * with a key it is no longer entitled to.
     *
     * A fetch that fails leaves the list empty and says nothing on screen. The
     * source does the same, and it is the right silence: nobody asked for these
     * models, they are an addition to a local model that either exists or does
     * not, and an error banner over a chat would be reporting the failure of
     * something the user never started.
     */
    public fun bindCloudSettings(source: CloudSettingsSource?) {
        accountModelsJob?.cancel()
        cloudSettings = source
        if (source == null) {
            config.replaceAccountModels(emptyList())
            publishModels()
            return
        }
        accountModelsJob = scope.launch {
            val models = try {
                source.plaintext()?.let(GeneralChatCloudConfigPolicy::models).orEmpty()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Throwable) {
                emptyList()
            }
            config.replaceAccountModels(models)
            publishModels()
            accountModelsJob = null
        }
    }

    /**
     * Republish whatever the catalog now says.
     *
     * Separate from [publish] because the two are triggered by different things
     * — one by a turn, one by the account or the provider form — and folding
     * them together would make every streamed delta re-read the keystore.
     */
    private fun publishModels() {
        _state.value = _state.value.copy(
            configured = config.activeEndpoint() != null,
            models = config.catalog(),
            activeModelId = config.activeModelId(),
        )
    }

    private fun saveConfig(intent: GeneralChatIntent.SaveConfig) {
        val current = _state.value.config
        val invalid = GeneralChatConfigValidator.validate(
            baseUrl = intent.baseUrl,
            model = intent.model,
            apiKey = intent.apiKey,
            clearApiKey = intent.clearApiKey,
            hasExistingApiKey = current.hasApiKey,
        )
        if (invalid != null) {
            _state.value = _state.value.copy(configFailure = invalid)
            return
        }
        try {
            config.save(intent.baseUrl, intent.model, intent.apiKey, intent.clearApiKey)
        } catch (_: SecureStorageUnavailable) {
            _state.value = _state.value.copy(
                configFailure = GeneralChatConfigFailure.SECURE_STORAGE,
            )
            return
        }
        // Deliberately leaves the transcript alone: editing the model name must
        // not throw away a conversation that is already on screen.
        val saved = config.snapshot()
        val configured = config.activeEndpoint() != null
        _state.value = _state.value.copy(
            configured = configured,
            config = saved,
            models = config.catalog(),
            activeModelId = config.activeModelId(),
            configFailure = null,
            failure = if (configured && _state.value.failure == GeneralChatFailureReason.UNCONFIGURED) {
                null
            } else {
                _state.value.failure
            },
        )
    }

    /**
     * Sends one throwaway turn to the draft's endpoint and reports what came
     * back, saving nothing either way.
     *
     * The validator runs first because a probe against a malformed URL would
     * report a network failure for what is really a typo, and the two need
     * different fixes. "Clear the key" is refused rather than probed for the
     * same reason the source refuses it: there would be no credential to send.
     */
    private fun testConnection(intent: GeneralChatIntent.TestConnection) {
        if (_state.value.connectionTest.running) return
        val current = _state.value.config
        val invalid = GeneralChatConfigValidator.validate(
            baseUrl = intent.baseUrl,
            model = intent.model,
            apiKey = intent.apiKey,
            clearApiKey = intent.clearApiKey,
            hasExistingApiKey = current.hasApiKey,
        )
        if (invalid != null || intent.clearApiKey) {
            _state.value = _state.value.copy(
                configFailure = invalid ?: GeneralChatConfigFailure.API_KEY_REQUIRED,
                connectionTest = GeneralChatConnectionTestUi(),
            )
            return
        }
        // Read once, held only for the length of the request, and never put
        // back into the state the UI observes.
        val apiKey = intent.apiKey.trim().ifEmpty { config.apiKey() }
        if (apiKey.isEmpty()) {
            _state.value = _state.value.copy(
                configFailure = GeneralChatConfigFailure.API_KEY_REQUIRED,
                connectionTest = GeneralChatConnectionTestUi(),
            )
            return
        }
        probe?.cancel()
        _state.value = _state.value.copy(
            configFailure = null,
            connectionTest = GeneralChatConnectionTestUi(true, false, null),
        )
        probe = scope.launch {
            try {
                stream.stream(
                    GeneralChatStreamRequest(
                        baseUrl = intent.baseUrl,
                        apiKey = apiKey,
                        model = intent.model,
                        messages = listOf(ModelProviderMessage("user", PROBE_PROMPT)),
                        maxTokens = PROBE_TOKEN_LIMIT,
                        systemPrompt = PROBE_SYSTEM_PROMPT,
                    ),
                ) { }
                _state.value = _state.value.copy(
                    connectionTest = GeneralChatConnectionTestUi(false, true, null),
                )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                _state.value = _state.value.copy(
                    connectionTest = GeneralChatConnectionTestUi(false, false, failureReason(error)),
                )
            } finally {
                probe = null
            }
        }
    }

    private fun updateDraft(text: String) {
        val id = draftId(sessionId)
        if (text.isEmpty()) drafts.delete(id) else drafts.save(id, text)
        _state.value = _state.value.copy(draft = text)
    }

    private fun setImages(images: List<ComposerImage>) {
        if (_state.value.busy) return
        _state.value = _state.value.copy(images = images.take(MAX_ATTACHMENTS))
    }

    private fun selectModel(modelId: String) {
        val selected = try {
            config.selectModel(modelId)
        } catch (_: SecureStorageUnavailable) {
            _state.value = _state.value.copy(
                configFailure = GeneralChatConfigFailure.SECURE_STORAGE,
            )
            return
        }
        if (!selected) return
        publishModels()
    }

    private fun newSession() {
        // An untouched blank session is already "new"; making another one would
        // just leave a trail of empty rows in the sidebar.
        if (timeline.snapshot().persistedMessages.isEmpty() && chats.loadSession(sessionId) == null) {
            return
        }
        cancel()
        openSession(newSessionId())
    }

    private fun selectSession(target: String) {
        if (target == sessionId) return
        cancel()
        openSession(target)
    }

    private fun openSession(target: String) {
        sessionId = target
        timeline.reset(target)
        timeline.setPersistedMessages(chats.loadMessages(target).map(::restoreMessage))
        _state.value = _state.value.copy(
            sessionId = target,
            sessions = listSessions(),
            timeline = timeline.snapshot(),
            messages = projectMessages(),
            draft = drafts.load(draftId(target)).orEmpty(),
            images = emptyList(),
            busy = false,
            failure = null,
            export = null,
        )
    }

    private fun deleteSession(target: String) {
        chats.deleteSession(target)
        drafts.delete(draftId(target))
        if (target != sessionId) {
            _state.value = _state.value.copy(sessions = listSessions())
            return
        }
        stop()
        val remaining = listSessions()
        openSession(remaining.firstOrNull()?.id ?: newSessionId())
    }

    private fun renameSession(target: String, title: String) {
        val existing = chats.loadSession(target) ?: return
        val trimmed = title.trim().take(TITLE_LIMIT)
        if (trimmed.isEmpty()) return
        chats.saveSession(
            PersistedChatSession(
                sessionId = existing.sessionId,
                title = trimmed,
                agentType = existing.agentType,
                status = existing.status,
                updatedAt = existing.updatedAt,
                createdAt = existing.createdAt,
                messageCount = existing.messageCount,
                pinned = existing.pinned,
            ),
        )
        _state.value = _state.value.copy(sessions = listSessions())
    }

    private fun archiveSession(target: String, archived: Boolean) {
        if (chats.loadSession(target) == null) return
        chats.setSessionStatus(target, if (archived) ARCHIVED else READY)
        _state.value = _state.value.copy(sessions = listSessions())
    }

    private fun pinSession(target: String, pinned: Boolean) {
        if (chats.loadSession(target) == null) return
        chats.pinSession(AGENT_TYPE, target, pinned)
        _state.value = _state.value.copy(sessions = listSessions())
    }

    private fun exportSession(intent: GeneralChatIntent.ExportSession) {
        val messages = if (intent.sessionId == sessionId) {
            timeline.snapshot().persistedMessages
        } else {
            chats.loadMessages(intent.sessionId).map(::restoreMessage)
        }
        if (messages.isEmpty()) return
        val title = chats.loadSession(intent.sessionId)?.title?.takeIf { it.isNotBlank() }
            ?: intent.untitledLabel
        _state.value = _state.value.copy(
            export = GeneralChatExportUi(
                sessionId = intent.sessionId,
                title = title,
                markdown = GeneralChatExportFormatter.markdown(
                    title = title,
                    messages = messages,
                    userLabel = intent.userLabel,
                    assistantLabel = intent.assistantLabel,
                ),
            ),
        )
    }

    private fun send() {
        // Read before anything is written down: whichever model is active
        // decides both where this turn goes and whether it can go at all, and a
        // user message saved against an endpoint that turns out not to exist is
        // a transcript entry with no reply and no explanation.
        val endpoint = config.activeEndpoint()
        if (endpoint == null) {
            _state.value = _state.value.copy(failure = GeneralChatFailureReason.UNCONFIGURED)
            return
        }
        if (_state.value.busy) return
        val prompt = _state.value.draft.trim()
        val composerImages = _state.value.images
        if (prompt.isEmpty() && composerImages.isEmpty()) return
        val imageAttachments = composerImages.map { image ->
            ImageAttachment(name = image.id, dataUrl = image.dataUrl)
        }

        val turnSession = sessionId
        val userMessage = message(
            id = nextId("user"),
            role = "user",
            text = prompt,
            status = "sent",
            turnId = null,
            images = imageAttachments,
        )
        timeline.applyEvent(ConversationEvent.UserMessage(userMessage, true))
        chats.saveMessage(persistedMessage(userMessage, turnSession))
        drafts.delete(draftId(turnSession))
        val turnId = nextId("turn")
        timeline.applyEvent(ConversationEvent.TurnStarted(turnSession, turnId))
        publish(draft = "", images = emptyList(), busy = true, failure = null)
        saveSession()

        val messages = timeline.snapshot().persistedMessages
            .filter { it.role == "user" || it.role == "assistant" }
            .filter { it.text.isNotBlank() || !it.images.isNullOrEmpty() }
            .map { ModelProviderMessage(it.role, it.text, it.images.orEmpty()) }
        request = scope.launch {
            try {
                val result = stream.stream(
                    GeneralChatStreamRequest(
                        // Held only for the length of the request, so the key is
                        // never a field anyone can accidentally log or persist.
                        baseUrl = endpoint.baseUrl,
                        apiKey = endpoint.apiKey,
                        model = endpoint.model,
                        messages = messages,
                        maxTokens = REPLY_TOKEN_LIMIT,
                        systemPrompt = SYSTEM_PROMPT,
                    ),
                ) { delta ->
                    if (turnSession != sessionId) return@stream
                    timeline.applyEvent(ConversationEvent.AssistantDelta(turnSession, turnId, delta))
                    publish(busy = true)
                }
                val assistant = message(
                    id = result.responseId.ifEmpty { nextId("assistant") },
                    role = "assistant",
                    text = result.text,
                    status = "completed",
                    turnId = turnId,
                )
                chats.saveMessage(persistedMessage(assistant, turnSession))
                if (turnSession == sessionId) {
                    timeline.applyEvent(ConversationEvent.AssistantMessage(assistant))
                    saveSession()
                    publish(busy = false, failure = null)
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Throwable) {
                if (turnSession == sessionId) {
                    timeline.applyEvent(ConversationEvent.Error(error.message.orEmpty()))
                    publish(busy = false, failure = failureReason(error))
                }
            } finally {
                request = null
            }
        }
    }

    private fun cancel() {
        val active = timeline.activeTurnOrNull()
        request?.cancel()
        request = null
        if (active != null) {
            val cancelled = active.copy(status = "cancelled", text = active.text)
            timeline.applyEvent(ConversationEvent.AssistantMessage(cancelled))
            chats.saveMessage(persistedMessage(cancelled, sessionId))
            saveSession()
        }
        publish(busy = false)
    }

    private fun clearFailure() {
        _state.value = _state.value.copy(failure = null)
    }

    private fun publish(
        draft: String = _state.value.draft,
        images: List<ComposerImage> = _state.value.images,
        busy: Boolean = _state.value.busy,
        failure: GeneralChatFailureReason? = _state.value.failure,
    ) {
        _state.value = _state.value.copy(
            timeline = timeline.snapshot(),
            messages = projectMessages(),
            draft = draft,
            images = images,
            busy = busy,
            failure = failure,
        )
    }

    private fun nextId(prefix: String): String {
        sequence += 1
        return prefix + "-" + Clock.System.now().toEpochMilliseconds() + "-" + sequence
    }

    private fun newSessionId(): String = "general-chat-" + nextId("s")

    private fun draftId(session: String): String = DRAFT_PREFIX + session

    private fun listSessions(): List<GeneralChatSessionUi> =
        chats.listSessions(AGENT_TYPE).map { row ->
            GeneralChatSessionUi(
                id = row.sessionId,
                title = row.title,
                status = row.status,
                pinned = row.pinned,
                createdAt = row.createdAt,
                updatedAt = row.updatedAt,
                messageCount = row.messageCount,
            )
        }

    private fun projectMessages(): List<GeneralChatMessageUi> {
        val snapshot = timeline.snapshot()
        return (snapshot.persistedMessages + listOfNotNull(snapshot.activeTurn))
            .filter { it.text.isNotBlank() }
            .map { message ->
                GeneralChatMessageUi(
                    id = message.id,
                    role = message.role,
                    status = message.status,
                    blocks = MarkdownParser.parse(message.text),
                )
            }
    }

    private fun saveSession() {
        val messages = timeline.snapshot().persistedMessages
        if (messages.isEmpty()) return
        val now = Clock.System.now().toString()
        val existing = chats.loadSession(sessionId)
        chats.saveSession(
            PersistedChatSession(
                sessionId = sessionId,
                // Only a brand new row gets a derived title, so a rename sticks.
                title = existing?.title ?: derivedTitle(messages),
                agentType = AGENT_TYPE,
                // Archiving is the user's decision about the whole conversation,
                // so a reply arriving in one does not quietly un-archive it.
                status = when {
                    existing?.status == ARCHIVED -> ARCHIVED
                    _state.value.busy -> ACTIVE
                    else -> READY
                },
                updatedAt = now,
                createdAt = existing?.createdAt ?: messages.first().timestamp ?: now,
                messageCount = messages.size,
                pinned = existing?.pinned == true,
            ),
        )
        _state.value = _state.value.copy(sessions = listSessions())
    }

    private fun derivedTitle(messages: List<ChatMessage>): String =
        messages.firstOrNull { it.role == "user" }?.text?.trim()?.take(TITLE_LIMIT).orEmpty()

    private fun persistedMessage(message: ChatMessage, session: String): PersistedChatMessage =
        PersistedChatMessage(
            messageId = message.id,
            sessionId = session,
            role = message.role,
            text = message.text,
            status = message.status,
            timestamp = message.timestamp,
            thinking = message.thinking,
            payloadJson = STORAGE_JSON.encodeToString(
                StoredMessagePayload(
                    renderVersion = message.renderVersion,
                    turnId = message.turnId,
                    detail = message.detail,
                    images = message.images.orEmpty(),
                ),
            ),
        )

    private fun restoreMessage(message: PersistedChatMessage): ChatMessage {
        val payload = try {
            STORAGE_JSON.decodeFromString<StoredMessagePayload>(message.payloadJson)
        } catch (_: Throwable) {
            StoredMessagePayload(null, null, null, emptyList())
        }
        return ChatMessage(
            id = message.messageId,
            role = message.role,
            text = message.text,
            status = message.status,
            renderVersion = payload.renderVersion,
            turnId = payload.turnId,
            detail = payload.detail,
            timestamp = message.timestamp,
            thinking = message.thinking,
            tools = null,
            items = null,
            images = payload.images,
        )
    }

    private fun failureReason(error: Throwable): GeneralChatFailureReason {
        val provider = (error as? ModelProviderException)?.failure
        return when (provider) {
            ModelProviderFailure.Authentication -> GeneralChatFailureReason.AUTHENTICATION
            ModelProviderFailure.RateLimited -> GeneralChatFailureReason.RATE_LIMITED
            ModelProviderFailure.ServiceUnavailable -> GeneralChatFailureReason.SERVICE_UNAVAILABLE
            ModelProviderFailure.InvalidResponse -> GeneralChatFailureReason.INVALID_RESPONSE
            is ModelProviderFailure.Http, null -> GeneralChatFailureReason.NETWORK
        }
    }

    public companion object {
        internal fun create(
            scope: CoroutineScope,
            stream: GeneralChatStreamPort,
            drafts: DraftStore,
            chats: ChatLocalStore,
            secure: SecureStore,
        ): GeneralChatStore = GeneralChatStore(scope, stream, drafts, chats, secure)

        internal fun providerStream(): GeneralChatStreamPort {
            val client = ModelProviderStreamClient.create()
            return GeneralChatStreamPort { request, onDelta ->
                client.stream(
                    baseUrl = request.baseUrl,
                    apiKey = request.apiKey,
                    model = request.model,
                    messages = request.messages,
                    systemPrompt = request.systemPrompt,
                    maxTokens = request.maxTokens,
                    onDelta = onDelta,
                )
            }
        }

        private fun message(
            id: String,
            role: String,
            text: String,
            status: String,
            turnId: String?,
            images: List<ImageAttachment> = emptyList(),
        ): ChatMessage = ChatMessage(
            id = id,
            role = role,
            text = text,
            status = status,
            renderVersion = null,
            turnId = turnId,
            detail = null,
            timestamp = Clock.System.now().toString(),
            thinking = null,
            tools = null,
            items = null,
            images = images,
        )

        private const val AGENT_TYPE = "general_chat"
        private const val ACTIVE = "active"
        private const val READY = "ready"
        private const val ARCHIVED = "archived"
        private const val DRAFT_PREFIX = "general-chat-composer:"
        private const val TITLE_LIMIT = 80
        private const val MAX_ATTACHMENTS = 4
        /** What a real reply may run to; the source's `buildRequestBody(..., 4096, ...)`. */
        private const val REPLY_TOKEN_LIMIT: Int = 4096

        /**
         * The probe's whole budget. One token is enough to prove the endpoint
         * answered, and anything more is spending the user's quota to learn
         * nothing — `probeConfiguration` asks for exactly this.
         */
        private const val PROBE_TOKEN_LIMIT: Int = 1
        private const val PROBE_PROMPT: String = "ping"
        private const val PROBE_SYSTEM_PROMPT: String =
            "You verify whether a model endpoint is reachable. Reply with one short word."

        private val SYSTEM_PROMPT = listOf(
            "You are the general-purpose AI work assistant in the OpenBitFun mobile app.",
            "OpenBitFun is a local AI workbench built around a Code Agent for long-horizon engineering and productivity tasks.",
            "The mobile app provides general chat plus entry points to OpenBitFun Remote/Code; this chat itself does not have local workspace access.",
            "It is not a blockchain, gaming, NFT, cryptocurrency, or Web3 product. Never invent those product claims.",
            "Treat those product exclusions as internal factual constraints and do not mention them unless the user asks something directly related.",
            "Help with general questions, writing, summarization, planning, research preparation, and everyday work.",
            "The user may attach images. When images are present, inspect them and answer from what you can see.",
            "This mobile chat currently has no local workspace, shell, desktop file, web browsing, or external tool access.",
            "Never claim to have used unavailable tools or accessed information you were not given.",
            "When a task requires reading or changing a local repository, running commands, or accessing desktop files, explain that the user should use OpenBitFun Remote/Code.",
            "Answer in the user's language. Be concise, concrete, and honest about uncertainty.",
        ).joinToString("\n")
        private val STORAGE_JSON = Json { ignoreUnknownKeys = true }
    }
}

@Serializable
private data class StoredMessagePayload(
    val renderVersion: Int?,
    val turnId: String?,
    val detail: String?,
    val images: List<ImageAttachment> = emptyList(),
)
