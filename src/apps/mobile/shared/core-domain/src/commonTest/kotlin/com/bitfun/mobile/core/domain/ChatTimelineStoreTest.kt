package com.bitfun.mobile.core.domain

import com.bitfun.mobile.core.protocol.ChatMessageItemResponse
import com.bitfun.mobile.core.protocol.RemoteToolStatusResponse
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class ChatTimelineStoreTest {
    @Test
    fun ownsOptimisticMergeAndActiveTurnCleanupRules() {
        val store = ChatTimelineStore()
        store.reset("session-1")
        store.appendOptimisticMessage(message("msg-local-1", "user", "Please edit"))
        store.setActiveTurn(message("active-turn-1", "assistant", "Final text", "completed"))
        store.mergePersistedMessages(listOf(message("remote-user-1", "user", "Please edit")))

        var state = store.snapshot()
        assertEquals(1, state.persistedMessages.size)
        assertEquals(0, state.optimisticMessages.size)
        assertEquals("active-turn-1", state.activeTurn?.id)
        assertEquals(ChatSyncPhase.FINALIZING, state.syncPhase)

        store.mergePersistedMessages(listOf(message("assistant-final-1", "assistant", "Final text")))
        state = store.snapshot()
        assertEquals(2, state.persistedMessages.size)
        assertEquals(null, state.activeTurn)
    }

    @Test
    fun appliesTransportNeutralTurnEvents() {
        val store = ChatTimelineStore()
        store.reset("session-events")
        store.applyEvent(ConversationEvent.TurnStarted("session-events", "turn-1"))
        store.applyEvent(ConversationEvent.AssistantDelta("session-events", "turn-1", "Hello"))
        store.applyEvent(ConversationEvent.AssistantDelta("session-events", "turn-1", " world"))

        val state = store.snapshot()
        assertEquals("Hello world", state.activeTurn?.text)
        assertEquals("turn-1", state.activeTurn?.turnId)
        assertEquals(ChatSyncPhase.STREAMING, state.syncPhase)
    }

    @Test
    fun ignoresEventsFromDifferentSession() {
        val store = ChatTimelineStore()
        store.reset("session-current")
        store.applyEvent(ConversationEvent.TurnStarted("session-other", "turn-other"))
        store.applyEvent(ConversationEvent.AssistantDelta("session-other", "turn-other", "stale"))

        assertEquals(null, store.snapshot().activeTurn)
    }

    @Test
    fun replacesPendingActiveTurnWithRemoteTurn() {
        val store = ChatTimelineStore()
        store.reset("session-1")
        val pendingId = store.setPendingActiveTurn("msg-local-1")

        assertEquals("active-pending-msg-local-1", pendingId)
        assertEquals(ChatSyncPhase.STREAMING, store.snapshot().syncPhase)
        store.setLocalActiveTurn("turn-remote-1")

        assertEquals("active-turn-remote-1", store.snapshot().activeTurn?.id)
        assertEquals("turn-remote-1", store.snapshot().activeTurn?.turnId)
    }

    @Test
    fun keepsStreamedTextFromGoingBackwards() {
        val store = ChatTimelineStore()
        store.reset("session-1")
        store.setActiveTurn(activeMessage("turn-stream-1", "abcdef"))
        store.setActiveTurn(activeMessage("turn-stream-1", "abc", "active", 2))

        val state = store.snapshot()
        assertEquals("abcdef", state.activeTurn?.text)
        assertEquals("turn-stream-1", state.activeTurn?.turnId)
        assertEquals(2, state.activeTurn?.renderVersion)
    }

    @Test
    fun monotonicallyMergesStructuredTextItems() {
        val store = ChatTimelineStore()
        store.reset("session-1")
        val first = activeMessage("turn-items-1", "")
            .copy(items = listOf(ChatMessageItemResponse(type = "text", content = "abcdef")))
        val second = activeMessage("turn-items-1", "", "active", 2)
            .copy(items = listOf(ChatMessageItemResponse(type = "text", content = "abc")))

        store.setActiveTurn(first)
        store.setActiveTurn(second)

        assertEquals("abcdef", store.snapshot().activeTurn?.items?.firstOrNull()?.content)
    }

    @Test
    fun updatesToolStatusFromLatestStructuredSnapshot() {
        val store = ChatTimelineStore()
        store.reset("session-1")
        val first = activeMessage("turn-tool-1", "").copy(
            items = listOf(
                ChatMessageItemResponse(
                    type = "tool",
                    tool = RemoteToolStatusResponse(id = "tool-1", name = "read_file", status = "running"),
                ),
            ),
        )
        val second = activeMessage("turn-tool-1", "", "active", 2).copy(
            items = listOf(
                ChatMessageItemResponse(
                    type = "tool",
                    tool = RemoteToolStatusResponse(
                        id = "tool-1",
                        name = "read_file",
                        status = "completed",
                        durationMs = 50,
                    ),
                ),
            ),
        )

        store.setActiveTurn(first)
        store.setActiveTurn(second)

        val tool = store.snapshot().activeTurn?.items?.firstOrNull()?.tool
        assertEquals("completed", tool?.status)
        assertEquals(50, tool?.durationMs)
    }

    @Test
    fun clearsActiveTurnWhenPersistedAssistantMatchesTurn() {
        val store = ChatTimelineStore()
        store.reset("session-1")
        store.setActiveTurn(activeMessage("turn-final-store-1", "partial active text", "completed", 0))
        store.mergePersistedMessages(listOf(message("turn-final-store-1_assistant", "assistant", "rewritten final text")))

        assertEquals(1, store.snapshot().persistedMessages.size)
        assertEquals(null, store.snapshot().activeTurn)
    }

    @Test
    fun holdsCompletedTurnUntilPersistedAssistantHasFinalContent() {
        val store = ChatTimelineStore()
        store.reset("session-1")
        store.setActiveTurn(activeMessage("turn-final-wait-1", "final answer", "completed", 0))
        store.mergePersistedMessages(
            listOf(message("turn-final-wait-1_assistant", "assistant", "Still reasoning", thinking = "Still reasoning")),
        )
        store.setActiveTurn(null)

        assertEquals("active-turn-final-wait-1", store.snapshot().activeTurn?.id)
        assertEquals(ChatSyncPhase.FINALIZING, store.snapshot().syncPhase)
        store.mergePersistedMessages(listOf(message("turn-final-wait-1_assistant", "assistant", "final answer")))
        assertEquals(null, store.snapshot().activeTurn)
    }

    @Test
    fun filtersSeedMessagesWhenSettingPersistedHistory() {
        val store = ChatTimelineStore()
        store.reset("session-1")
        store.setPersistedMessages(
            listOf(
                message("system-seed", "assistant", "Welcome", "ready"),
                message("user-1", "user", "Hello"),
            ),
        )

        assertEquals(listOf("user-1"), store.snapshot().persistedMessages.map { it.id })
    }

    @Test
    fun eventErrorMovesStoreToErrorPhase() {
        val store = ChatTimelineStore()
        store.reset("session-1")
        store.applyEvent(ConversationEvent.Error("transport failure"))

        assertEquals(ChatSyncPhase.ERROR, store.snapshot().syncPhase)
        assertTrue(store.project(false).isNotEmpty())
        assertFalse(store.snapshot().selectedModelId.isNotEmpty())
    }

    private fun message(
        id: String,
        role: String,
        text: String,
        status: String = if (role == "assistant") "done" else "sent",
        thinking: String? = null,
    ): ChatMessage = ChatMessage(
        id = id,
        role = role,
        text = text,
        status = status,
        renderVersion = null,
        turnId = null,
        detail = null,
        timestamp = null,
        thinking = thinking,
        tools = null,
        items = null,
        images = null,
    )

    private fun activeMessage(
        turnId: String,
        text: String,
        status: String = "active",
        renderVersion: Int? = null,
    ): ChatMessage = ChatMessage(
        id = "active-$turnId",
        role = "assistant",
        text = text,
        status = status,
        renderVersion = renderVersion,
        turnId = turnId,
        detail = null,
        timestamp = null,
        thinking = null,
        tools = null,
        items = null,
        images = null,
    )
}
