package com.openbitfun.mobile.core.feature.generalchat

import com.openbitfun.mobile.core.persistence.ChatLocalStore
import com.openbitfun.mobile.core.persistence.DraftStore
import com.openbitfun.mobile.core.persistence.PersistedChatMessage
import com.openbitfun.mobile.core.persistence.PersistedChatSession
import com.openbitfun.mobile.core.persistence.SecureStore
import com.openbitfun.mobile.core.feature.session.ComposerImage
import com.openbitfun.mobile.core.transport.ModelProviderException
import com.openbitfun.mobile.core.transport.ModelProviderFailure
import com.openbitfun.mobile.core.transport.ModelProviderStreamResult
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class GeneralChatStoreTest {
    @Test
    fun savedConfigurationSurvivesRestartWithoutExposingTheKey() = runTest {
        val secure = MemorySecure()
        val first = store(this, secure = secure)
        first.dispatch(configure())

        assertTrue(first.state.value.configured)
        assertTrue(first.state.value.config.hasApiKey)
        assertFalse(first.state.value.toString().contains("secret-key"))
        assertFalse(configure().toString().contains("secret-key"))

        val restarted = store(this, secure = secure)
        assertTrue(restarted.state.value.configured)
        assertEquals("https://api.example.com", restarted.state.value.config.baseUrl)
        assertEquals("model-1", restarted.state.value.config.model)
        assertTrue(restarted.state.value.config.hasApiKey)
    }

    @Test
    fun blankKeyKeepsTheStoredOneAndClearRemovesIt() = runTest {
        val secure = MemorySecure()
        val store = store(this, secure = secure)
        store.dispatch(configure())

        store.dispatch(GeneralChatIntent.SaveConfig("https://api.example.com", "model-2", "", false))
        assertNull(store.state.value.configFailure)
        assertEquals("model-2", store.state.value.config.model)
        assertTrue(store.state.value.config.hasApiKey)

        store.dispatch(GeneralChatIntent.SaveConfig("https://api.example.com", "model-2", "", true))
        assertEquals(GeneralChatConfigFailure.API_KEY_REQUIRED, store.state.value.configFailure)
        assertTrue(store.state.value.config.hasApiKey)
    }

    @Test
    fun invalidConfigurationIsRefusedWithATypedReason() = runTest {
        val store = store(this)

        store.dispatch(GeneralChatIntent.SaveConfig("api.example.com", "model-1", "k", false))
        assertEquals(GeneralChatConfigFailure.INVALID_URL, store.state.value.configFailure)

        store.dispatch(GeneralChatIntent.SaveConfig("https://api.example.com", " ", "k", false))
        assertEquals(GeneralChatConfigFailure.MODEL_REQUIRED, store.state.value.configFailure)

        store.dispatch(GeneralChatIntent.ClearConfigFailure)
        assertNull(store.state.value.configFailure)
        assertFalse(store.state.value.configured)
    }

    @Test
    fun secureStorageFailureIsReportedInsteadOfCrashing() = runTest {
        val store = store(this, secure = MemorySecure(failWrites = true))

        store.dispatch(configure())

        assertEquals(GeneralChatConfigFailure.SECURE_STORAGE, store.state.value.configFailure)
        assertFalse(store.state.value.configured)
    }

    @Test
    fun secureReadFailureDuringConstructionIsFailClosedWithoutMutatingStorage() = runTest {
        val sessionId = "existing-session"
        val chats = MemoryChats().apply {
            sessions += PersistedChatSession(
                sessionId = sessionId,
                title = "Existing conversation",
                agentType = "general_chat",
                status = "ready",
                updatedAt = "2026-01-02T00:00:00Z",
                createdAt = "2026-01-01T00:00:00Z",
                messageCount = 1,
                pinned = false,
            )
            messages += PersistedChatMessage(
                messageId = "message-1",
                sessionId = sessionId,
                role = "user",
                text = "restored message",
                status = "sent",
                timestamp = "2026-01-01T00:00:00Z",
                thinking = null,
                payloadJson = "{}",
            )
        }
        val drafts = MemoryDrafts(
            mutableMapOf("general-chat-composer:$sessionId" to "restored draft"),
        )
        val secure = FailingReadSecure()

        val store = store(this, drafts = drafts, chats = chats, secure = secure)

        val state = store.state.value
        assertFalse(state.configured)
        assertEquals(GeneralChatConfigUi("", "", false), state.config)
        assertEquals(GeneralChatConfigFailure.SECURE_STORAGE, state.configFailure)
        assertEquals(emptyList(), state.models)
        assertEquals("", state.activeModelId)
        assertEquals(sessionId, state.sessionId)
        assertEquals("restored draft", state.draft)
        assertEquals(listOf("restored message"), state.timeline.persistedMessages.map { it.text })
        assertEquals(1, secure.reads)
        assertEquals(0, secure.writes)
        assertEquals(0, secure.deletes)
    }

    @Test
    fun sendPersistsComposerStateAndProjectsStreamedReply() = runTest {
        val drafts = MemoryDrafts()
        var recorded: GeneralChatStreamRequest? = null
        val stream = GeneralChatStreamPort { request, onDelta ->
            recorded = request
            onDelta("hel")
            onDelta("lo")
            ModelProviderStreamResult(200, "reply-1", "hello")
        }
        val chats = MemoryChats()
        val store = store(this, stream, drafts, chats)
        store.dispatch(configure())
        store.dispatch(GeneralChatIntent.UpdateDraft("question"))

        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()

        val state = store.state.value
        assertFalse(state.busy)
        assertEquals("", state.draft)
        assertNull(state.failure)
        assertEquals(listOf("user", "assistant"), state.timeline.persistedMessages.map { it.role })
        assertEquals(listOf("question", "hello"), state.timeline.persistedMessages.map { it.text })
        assertNull(state.timeline.activeTurn)
        assertNull(drafts.load("general-chat-composer:" + state.sessionId))
        assertEquals("secret-key", recorded?.apiKey)
        assertEquals(listOf("question"), recorded?.messages?.map { it.content })
        assertEquals(2, chats.messages.size)
        assertEquals(listOf(state.sessionId), state.sessions.map { it.id })
        assertEquals("question", state.sessions.single().title)
    }

    @Test
    fun imageOnlySendPersistsAndReplaysTheAttachment() = runTest {
        var recorded: GeneralChatStreamRequest? = null
        val stream = GeneralChatStreamPort { request, _ ->
            recorded = request
            ModelProviderStreamResult(200, "reply-1", "I can see it.")
        }
        val chats = MemoryChats()
        val secure = MemorySecure()
        val store = store(this, stream = stream, chats = chats, secure = secure)
        store.dispatch(configure())
        store.dispatch(
            GeneralChatIntent.SetImages(
                listOf(ComposerImage("image-1", "data:image/png;base64,AAAA", "image/png")),
            ),
        )

        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()

        val user = store.state.value.timeline.persistedMessages.first()
        assertEquals("", user.text)
        assertEquals("data:image/png;base64,AAAA", user.images?.single()?.dataUrl)
        assertEquals(emptyList(), store.state.value.images)
        assertEquals("data:image/png;base64,AAAA", recorded?.messages?.single()?.images?.single()?.dataUrl)

        val restarted = store(this, reply("unused"), chats = chats, secure = secure)
        assertEquals(
            "data:image/png;base64,AAAA",
            restarted.state.value.timeline.persistedMessages.first().images?.single()?.dataUrl,
        )
    }

    @Test
    fun draftsAndTranscriptsAreKeptPerSession() = runTest {
        val drafts = MemoryDrafts()
        val chats = MemoryChats()
        val store = store(this, reply("first-reply"), drafts, chats)
        store.dispatch(configure())
        store.dispatch(GeneralChatIntent.UpdateDraft("first question"))
        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()
        val first = store.state.value.sessionId

        store.dispatch(GeneralChatIntent.NewSession)
        val second = store.state.value.sessionId
        assertNotEquals(first, second)
        assertEquals(emptyList(), store.state.value.messages)
        store.dispatch(GeneralChatIntent.UpdateDraft("unsent"))

        store.dispatch(GeneralChatIntent.SelectSession(first))
        assertEquals(first, store.state.value.sessionId)
        assertEquals("", store.state.value.draft)
        assertEquals(
            listOf("first question", "first-reply"),
            store.state.value.timeline.persistedMessages.map { it.text },
        )

        store.dispatch(GeneralChatIntent.SelectSession(second))
        assertEquals("unsent", store.state.value.draft)
    }

    @Test
    fun renameSurvivesTheNextSend() = runTest {
        val chats = MemoryChats()
        val store = store(this, reply("ok"), chats = chats)
        store.dispatch(configure())
        store.dispatch(GeneralChatIntent.UpdateDraft("first"))
        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()
        val session = store.state.value.sessionId

        store.dispatch(GeneralChatIntent.RenameSession(session, "  Renamed  "))
        assertEquals("Renamed", store.state.value.sessions.single().title)

        store.dispatch(GeneralChatIntent.UpdateDraft("second"))
        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()

        assertEquals("Renamed", store.state.value.sessions.single().title)
    }

    @Test
    fun pinningIsExclusiveAndSurvivesTheNextSend() = runTest {
        val store = store(this, reply("ok"))
        store.dispatch(configure())
        val first = send(store, "first")
        val second = send(store, "second")

        store.dispatch(GeneralChatIntent.PinSession(first, true))
        assertEquals(listOf(first), pinnedIds(store))

        // The sidebar shows one pinned slot, so pinning the second has to release
        // the first rather than leave two rows with no order between them.
        store.dispatch(GeneralChatIntent.PinSession(second, true))
        assertEquals(listOf(second), pinnedIds(store))

        store.dispatch(GeneralChatIntent.SelectSession(second))
        store.dispatch(GeneralChatIntent.UpdateDraft("again"))
        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()

        assertEquals(listOf(second), pinnedIds(store))
    }

    @Test
    fun archivingSurvivesTheNextReplyAndCanBeUndone() = runTest {
        val store = store(this, reply("ok"))
        store.dispatch(configure())
        val session = send(store, "first")

        store.dispatch(GeneralChatIntent.ArchiveSession(session, true))
        assertEquals("archived", statusOf(store, session))

        // Archiving is a decision about the whole conversation; a reply arriving
        // in it is not a request to file it back out.
        store.dispatch(GeneralChatIntent.UpdateDraft("second"))
        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()
        assertEquals("archived", statusOf(store, session))

        store.dispatch(GeneralChatIntent.ArchiveSession(session, false))
        assertNotEquals("archived", statusOf(store, session))
    }

    @Test
    fun deletingTheOpenSessionFallsBackToTheNewestRemainingOne() = runTest {
        val drafts = MemoryDrafts()
        val chats = MemoryChats()
        val store = store(this, reply("ok"), drafts, chats)
        store.dispatch(configure())
        store.dispatch(GeneralChatIntent.UpdateDraft("first"))
        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()
        val first = store.state.value.sessionId
        store.dispatch(GeneralChatIntent.NewSession)
        store.dispatch(GeneralChatIntent.UpdateDraft("second"))
        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()
        val second = store.state.value.sessionId

        store.dispatch(GeneralChatIntent.DeleteSession(second))

        assertEquals(first, store.state.value.sessionId)
        assertEquals(listOf(first), store.state.value.sessions.map { it.id })
        assertTrue(chats.messages.none { it.sessionId == second })
        assertNull(drafts.load("general-chat-composer:$second"))

        store.dispatch(GeneralChatIntent.DeleteSession(first))
        assertEquals(emptyList(), store.state.value.sessions)
        assertNotEquals(first, store.state.value.sessionId)
        assertEquals(emptyList(), store.state.value.messages)
    }

    @Test
    fun exportRendersMarkdownWithCallerSuppliedLabels() = runTest {
        val store = store(this, reply("an answer"))
        store.dispatch(configure())
        store.dispatch(GeneralChatIntent.UpdateDraft("a question"))
        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()
        val session = store.state.value.sessionId

        store.dispatch(GeneralChatIntent.ExportSession(session, "Untitled", "Me", "Assistant"))

        val export = store.state.value.export
        assertEquals(session, export?.sessionId)
        assertEquals(
            "# a question\n\n## Me\n\na question\n\n## Assistant\n\nan answer\n",
            export?.markdown,
        )

        store.dispatch(GeneralChatIntent.ClearExport)
        assertNull(store.state.value.export)
    }

    @Test
    fun cancelKeepsPartialAssistantTextAsCancelledMessage() = runTest {
        val started = CompletableDeferred<Unit>()
        val stream = GeneralChatStreamPort { _, onDelta ->
            onDelta("partial")
            started.complete(Unit)
            awaitCancellation()
        }
        val store = store(this, stream)
        store.dispatch(configure())
        store.dispatch(GeneralChatIntent.UpdateDraft("question"))
        store.dispatch(GeneralChatIntent.Send)
        runCurrent()
        started.await()

        store.dispatch(GeneralChatIntent.Cancel)
        runCurrent()

        val assistant = store.state.value.timeline.persistedMessages.last()
        assertEquals("assistant", assistant.role)
        assertEquals("partial", assistant.text)
        assertEquals("cancelled", assistant.status)
        assertFalse(store.state.value.busy)
        assertNull(store.state.value.timeline.activeTurn)
    }

    @Test
    fun providerFailureMapsToTypedUiFailure() = runTest {
        val stream = GeneralChatStreamPort { _, _ ->
            throw ModelProviderException(ModelProviderFailure.RateLimited)
        }
        val store = store(this, stream)
        store.dispatch(configure())
        store.dispatch(GeneralChatIntent.UpdateDraft("question"))

        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()

        assertEquals(GeneralChatFailureReason.RATE_LIMITED, store.state.value.failure)
        assertFalse(store.state.value.busy)
    }

    @Test
    fun sendingWithoutConfigurationReportsUnconfigured() = runTest {
        val store = store(this)
        store.dispatch(GeneralChatIntent.UpdateDraft("question"))

        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()

        assertEquals(GeneralChatFailureReason.UNCONFIGURED, store.state.value.failure)

        store.dispatch(configure())
        assertNull(store.state.value.failure)
    }

    @Test
    fun selectedModelIsPersistedByIdAndUsedForTheNextTurn() = runTest {
        val secure = MemorySecure()
        val first = store(this, secure = secure)
        first.dispatch(configure())
        first.dispatch(
            GeneralChatIntent.SelectModel(GeneralChatConfigStore.LOCAL_MODEL_ID),
        )
        assertEquals(GeneralChatConfigStore.LOCAL_MODEL_ID, first.state.value.activeModelId)

        val restarted = store(this, secure = secure)
        assertEquals(GeneralChatConfigStore.LOCAL_MODEL_ID, restarted.state.value.activeModelId)
        assertFalse(restarted.state.value.toString().contains("secret-key"))
    }

    @Test
    fun selectingAnUnknownModelDoesNotChangeTheActiveModel() = runTest {
        val store = store(this)
        store.dispatch(configure())
        val before = store.state.value.activeModelId

        store.dispatch(GeneralChatIntent.SelectModel("missing-model"))

        assertEquals(before, store.state.value.activeModelId)
    }

    @Test
    fun aSelectionWriteFailureKeepsTheCurrentModelAndReportsIt() = runTest {
        val secure = MemorySecure()
        val store = store(this, secure = secure)
        store.dispatch(configure())
        val before = store.state.value.activeModelId
        secure.failWrites = true

        store.dispatch(GeneralChatIntent.SelectModel(GeneralChatConfigStore.LOCAL_MODEL_ID))

        assertEquals(before, store.state.value.activeModelId)
        assertEquals(GeneralChatConfigFailure.SECURE_STORAGE, store.state.value.configFailure)
    }

    /**
     * The probe reaches the endpoint the user is typing, not the one on disk,
     * and does not write either — which is the whole reason to have a separate
     * button rather than "save and see what happens".
     */
    @Test
    fun testConnectionProbesTheDraftWithTheStoredKeyAndSavesNothing() = runTest {
        val secure = MemorySecure()
        var recorded: GeneralChatStreamRequest? = null
        val stream = GeneralChatStreamPort { request, _ ->
            recorded = request
            ModelProviderStreamResult(200, "", "pong")
        }
        val store = store(this, stream, secure = secure)
        store.dispatch(configure())

        store.dispatch(
            GeneralChatIntent.TestConnection("https://probe.example.com", "model-9", "", false),
        )
        advanceUntilIdle()

        assertTrue(store.state.value.connectionTest.passed)
        assertFalse(store.state.value.connectionTest.running)
        assertNull(store.state.value.connectionTest.failure)
        assertEquals("https://probe.example.com", recorded?.baseUrl)
        assertEquals("model-9", recorded?.model)
        // The blank field meant "the key I already saved", the same way it does
        // when saving — otherwise changing a model name would mean retyping it.
        assertEquals("secret-key", recorded?.apiKey)
        assertEquals(1, recorded?.maxTokens)
        // Nothing was written: the saved model is still the one from configure().
        assertEquals("model-1", store.state.value.config.model)
        assertEquals("https://api.example.com", store.state.value.config.baseUrl)
        assertFalse(store.state.value.toString().contains("secret-key"))
    }

    @Test
    fun aRefusedProbeIsReportedWithoutDisturbingTheSavedConfiguration() = runTest {
        val stream = GeneralChatStreamPort { _, _ ->
            throw ModelProviderException(ModelProviderFailure.Authentication)
        }
        val store = store(this, stream)
        store.dispatch(configure())

        store.dispatch(
            GeneralChatIntent.TestConnection("https://api.example.com", "model-1", "", false),
        )
        advanceUntilIdle()

        assertEquals(GeneralChatFailureReason.AUTHENTICATION, store.state.value.connectionTest.failure)
        assertFalse(store.state.value.connectionTest.passed)
        assertTrue(store.state.value.configured)

        store.dispatch(GeneralChatIntent.ClearConnectionTest)
        assertNull(store.state.value.connectionTest.failure)
    }

    /**
     * A probe that could not carry a credential is a form error, not a network
     * one: the source refuses it before the request rather than letting the
     * endpoint answer 401 and blaming the endpoint.
     */
    @Test
    fun aProbeWithNoKeyToSendIsRefusedBeforeTheRequest() = runTest {
        val store = store(this)
        store.dispatch(configure())

        store.dispatch(
            GeneralChatIntent.TestConnection("https://api.example.com", "model-1", "", true),
        )
        advanceUntilIdle()

        assertEquals(GeneralChatConfigFailure.API_KEY_REQUIRED, store.state.value.configFailure)
        assertFalse(store.state.value.connectionTest.running)
        assertFalse(store.state.value.connectionTest.passed)

        store.dispatch(
            GeneralChatIntent.TestConnection("api.example.com", "model-1", "k", false),
        )
        advanceUntilIdle()

        assertEquals(GeneralChatConfigFailure.INVALID_URL, store.state.value.configFailure)
        assertFalse(store.state.value.connectionTest.passed)
    }

    /** Starts a fresh conversation, sends one message, and names it. */
    private fun TestScope.send(store: GeneralChatStore, text: String): String {
        store.dispatch(GeneralChatIntent.NewSession)
        store.dispatch(GeneralChatIntent.UpdateDraft(text))
        store.dispatch(GeneralChatIntent.Send)
        advanceUntilIdle()
        return store.state.value.sessionId
    }

    private fun pinnedIds(store: GeneralChatStore): List<String> =
        store.state.value.sessions.filter { it.pinned }.map { it.id }

    private fun statusOf(store: GeneralChatStore, sessionId: String): String? =
        store.state.value.sessions.firstOrNull { it.id == sessionId }?.status

    private fun store(
        scope: CoroutineScope,
        stream: GeneralChatStreamPort = GeneralChatStreamPort { _, _ -> error("unused") },
        drafts: DraftStore = MemoryDrafts(),
        chats: ChatLocalStore = MemoryChats(),
        secure: SecureStore = MemorySecure(),
    ): GeneralChatStore = GeneralChatStore.create(scope, stream, drafts, chats, secure)

    private fun configure(): GeneralChatIntent.SaveConfig =
        GeneralChatIntent.SaveConfig("https://api.example.com", "model-1", "secret-key", false)

    private fun reply(text: String): GeneralChatStreamPort = GeneralChatStreamPort { _, _ ->
        ModelProviderStreamResult(200, "", text)
    }
}

private class MemoryChats : ChatLocalStore {
    val messages = mutableListOf<PersistedChatMessage>()
    val sessions = mutableListOf<PersistedChatSession>()

    override fun listSessions(agentType: String): List<PersistedChatSession> =
        sessions.filter { it.agentType == agentType }.sortedByDescending { it.updatedAt }

    override fun loadSession(sessionId: String): PersistedChatSession? =
        sessions.firstOrNull { it.sessionId == sessionId }

    override fun loadMessages(sessionId: String): List<PersistedChatMessage> =
        messages.filter { it.sessionId == sessionId }

    override fun saveSession(session: PersistedChatSession) {
        sessions.removeAll { it.sessionId == session.sessionId }
        sessions += session
    }

    override fun saveMessage(message: PersistedChatMessage) {
        messages.removeAll { it.messageId == message.messageId }
        messages += message
    }

    override fun pinSession(agentType: String, sessionId: String, pinned: Boolean) {
        sessions.indices.forEach { index ->
            val session = sessions[index]
            if (session.agentType != agentType) return@forEach
            sessions[index] = session.copy(pinned = pinned && session.sessionId == sessionId)
        }
    }

    override fun setSessionStatus(sessionId: String, status: String) {
        sessions.indices.forEach { index ->
            val session = sessions[index]
            if (session.sessionId == sessionId) sessions[index] = session.copy(status = status)
        }
    }

    override fun deleteSession(sessionId: String) {
        sessions.removeAll { it.sessionId == sessionId }
        messages.removeAll { it.sessionId == sessionId }
    }
}

private class MemoryDrafts(
    private val values: MutableMap<String, String> = mutableMapOf(),
) : DraftStore {
    override fun load(draftId: String): String? = values[draftId]

    override fun save(draftId: String, text: String) {
        values[draftId] = text
    }

    override fun delete(draftId: String) {
        values.remove(draftId)
    }
}

private class FailingReadSecure : SecureStore {
    var reads = 0
    var writes = 0
    var deletes = 0

    override fun read(key: String): ByteArray? {
        reads += 1
        error("keystore unavailable")
    }

    override fun write(key: String, value: ByteArray) {
        writes += 1
    }

    override fun delete(key: String) {
        deletes += 1
    }
}

private class MemorySecure(var failWrites: Boolean = false) : SecureStore {
    private val values = mutableMapOf<String, ByteArray>()

    override fun read(key: String): ByteArray? = values[key]

    override fun write(key: String, value: ByteArray) {
        if (failWrites) error("keystore unavailable")
        values[key] = value
    }

    override fun delete(key: String) {
        values.remove(key)
    }
}
