package com.openbitfun.mobile.core.domain

import com.openbitfun.mobile.core.protocol.ImageAttachment
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class ChatTimelineProjectorTest {
    @Test
    fun filtersSeedMessagesAndReturnsEmptyStateForEmptyRealHistory() {
        val items = ChatTimelineProjector.project(
            messages = listOf(message("system-seed", "assistant", "Welcome")),
            pendingMessages = emptyList(),
            activeTurn = null,
            hasMoreMessages = false,
        )

        assertEquals(1, items.size)
        assertEquals(ChatTimelineItemType.EMPTY_STATE, items[0].type)
    }

    @Test
    fun deduplicatesOptimisticUserMessagesReplacedByPersistedMessages() {
        val items = ChatTimelineProjector.project(
            messages = listOf(message("remote-user-1", "user", "Explain this file")),
            pendingMessages = listOf(message("remote-user-1", "user", " Explain this file ")),
            activeTurn = null,
            hasMoreMessages = false,
        )

        assertEquals(1, items.size)
        assertEquals("message-remote-user-1", items[0].id)
        assertEquals(ChatTimelineItemType.USER_MESSAGE, items[0].type)
    }

    @Test
    fun projectorKeepsARepeatedPendingMessageWithDifferentIdentity() {
        val items = ChatTimelineProjector.project(
            messages = listOf(message("remote-user-1", "user", "Explain this file")),
            pendingMessages = listOf(message("msg-local-2", "user", "Explain this file")),
            activeTurn = null,
            hasMoreMessages = false,
        )

        assertEquals(2, items.size)
        assertEquals("pending-msg-local-2", items[1].id)
    }

    @Test
    fun keepsOptimisticImageMessagesWhenPersistedImagePayloadDiffers() {
        val items = ChatTimelineProjector.project(
            messages = listOf(
                messageWithImage(
                    id = "remote-user-1",
                    name = "remote.png",
                    dataUrl = "data:image/png;base64,remote",
                ),
            ),
            pendingMessages = listOf(
                messageWithImage(
                    id = "msg-local-1",
                    name = "local.png",
                    dataUrl = "data:image/png;base64,local",
                ),
            ),
            activeTurn = null,
            hasMoreMessages = false,
        )

        assertEquals(2, items.size)
        assertEquals("message-remote-user-1", items[0].id)
        assertEquals("pending-msg-local-1", items[1].id)
        assertEquals(ChatTimelineItemType.OPTIMISTIC_USER_MESSAGE, items[1].type)
    }

    @Test
    fun keepsActiveTurnUntilMatchingAssistantMessageIsPersisted() {
        val activeTurn = message(
            id = "active-turn-1",
            role = "assistant",
            text = "Done with the edit",
            status = "completed",
            turnId = "turn-1",
        )
        val beforeFinal = ChatTimelineProjector.project(
            messages = listOf(message("remote-user-1", "user", "Please edit")),
            pendingMessages = emptyList(),
            activeTurn = activeTurn,
            hasMoreMessages = false,
        )
        val afterFinal = ChatTimelineProjector.project(
            messages = listOf(
                message("remote-user-1", "user", "Please edit"),
                message("assistant-final-1", "assistant", "Done with the edit", turnId = "turn-1"),
            ),
            pendingMessages = emptyList(),
            activeTurn = activeTurn,
            hasMoreMessages = false,
        )

        assertEquals(2, beforeFinal.size)
        assertEquals(ChatTimelineItemType.ASSISTANT_LIVE_TURN, beforeFinal[1].type)
        assertTrue(beforeFinal[1].isFinalizing)
        assertEquals(2, afterFinal.size)
        assertEquals("message-assistant-final-1", afterFinal[1].id)
        assertEquals(ChatTimelineItemType.ASSISTANT_MESSAGE, afterFinal[1].type)
    }

    @Test
    fun keepsActiveTurnItemIdStableWhenVisibleContentChanges() {
        val firstItems = ChatTimelineProjector.project(
            emptyList(),
            emptyList(),
            activeMessage("turn-stable-1", "abc", "active", 1),
            false,
        )
        val secondItems = ChatTimelineProjector.project(
            emptyList(),
            emptyList(),
            activeMessage("turn-stable-1", "abcdef", "active", 2),
            false,
        )

        assertEquals(1, firstItems.size)
        assertEquals(1, secondItems.size)
        assertEquals("active-turn-stable-1", firstItems[0].id)
        assertEquals(firstItems[0].id, secondItems[0].id)
    }

    @Test
    fun hidesActiveTurnWhenFinalAssistantIdMatchesTurnId() {
        val items = ChatTimelineProjector.project(
            messages = listOf(
                message("turn-final-1_assistant", "assistant", "rewritten final text"),
            ),
            pendingMessages = emptyList(),
            activeTurn = activeMessage("turn-final-1", "partial active text", "completed", 0),
            hasMoreMessages = false,
        )

        assertEquals(1, items.size)
        assertEquals("message-turn-final-1_assistant", items[0].id)
        assertEquals(ChatTimelineItemType.ASSISTANT_MESSAGE, items[0].type)
    }

    @Test
    fun keepsActiveTurnWhenMatchingPersistedAssistantHasNoFinalTextYet() {
        val pendingFinal = message(
            id = "turn-final-wait-1_assistant",
            role = "assistant",
            text = "Still reasoning",
            thinking = "Still reasoning",
        )
        val items = ChatTimelineProjector.project(
            messages = listOf(pendingFinal),
            pendingMessages = emptyList(),
            activeTurn = activeMessage("turn-final-wait-1", "final answer", "completed", 0),
            hasMoreMessages = false,
        )

        assertEquals(2, items.size)
        assertEquals(ChatTimelineItemType.ASSISTANT_LIVE_TURN, items[1].type)
        assertTrue(items[1].isFinalizing)
    }

    @Test
    fun offersRetryOnlyForLatestUnresolvedFailedMessage() {
        val interrupted = message(
            id = "assistant-failed-1",
            role = "assistant",
            text = "Partial reply",
            status = "failed",
            detail = "Retry prompt",
        )
        val unresolved = ChatTimelineProjector.project(
            listOf(message("user-1", "user", "Retry prompt"), interrupted),
            emptyList(),
            null,
            false,
        )
        val continued = ChatTimelineProjector.project(
            listOf(
                message("user-1", "user", "Retry prompt"),
                interrupted,
                message("user-2", "user", "Continue"),
            ),
            emptyList(),
            null,
            false,
        )

        assertTrue(unresolved[1].showRetryAction)
        assertFalse(continued[1].showRetryAction)
        assertFalse(continued[2].showRetryAction)
    }

    @Test
    fun keepsMessageAndToolStatusVocabulariesSeparate() {
        assertTrue(MessageStatusSemantics.shouldHoldCompletedTurn("done"))
        assertFalse(MessageStatusSemantics.isFinalizing("done"))
        assertTrue(MessageStatusSemantics.shouldHoldCompletedTurn("success"))
        assertTrue(ToolStatusSemantics.shouldKeepPrevious("finished", "running"))
        assertTrue(ToolStatusSemantics.shouldKeepPrevious("cancelled", "completed"))
        assertTrue(ToolStatusSemantics.shouldKeepPrevious("canceled", "running"))
        assertFalse(ToolStatusSemantics.shouldKeepPrevious("running", "finished"))
    }

    @Test
    fun placesActiveTurnAfterAnchoredOptimisticMessage() {
        val items = ChatTimelineProjector.project(
            messages = listOf(message("persisted-user", "user", "First")),
            pendingMessages = listOf(message("pending-local-1", "user", "Second")),
            activeTurn = activeMessage("turn-1", "Reply", "active", 1),
            hasMoreMessages = false,
            activeTurnAnchorId = "pending-local-1",
        )

        assertEquals(
            listOf(
                ChatTimelineItemType.USER_MESSAGE,
                ChatTimelineItemType.OPTIMISTIC_USER_MESSAGE,
                ChatTimelineItemType.ASSISTANT_LIVE_TURN,
            ),
            items.map { it.type },
        )
    }

    @Test
    fun placesActiveTurnBeforePendingWhenAnchorIsMissingOrStale() {
        val items = ChatTimelineProjector.project(
            messages = listOf(message("persisted-user", "user", "First")),
            pendingMessages = listOf(message("pending-local-2", "user", "Second")),
            activeTurn = activeMessage("turn-1", "Reply", "active", 1),
            hasMoreMessages = false,
            activeTurnAnchorId = "stale-local-id",
        )

        assertEquals(
            listOf("message-persisted-user", "active-turn-1", "pending-pending-local-2"),
            items.map { it.id },
        )
    }

    @Test
    fun staleAnchorFallsBackWithoutCrashing() {
        val items = ChatTimelineProjector.project(
            messages = emptyList(),
            pendingMessages = listOf(message("pending-local-3", "user", "Second")),
            activeTurn = activeMessage("turn-1", "Reply", "active", 1),
            hasMoreMessages = false,
            activeTurnAnchorId = "not-present",
        )

        assertEquals(listOf("active-turn-1", "pending-pending-local-3"), items.map { it.id })
    }

    @Test
    fun activeRowIdRemainsStableWhenInterleavedAfterAnchor() {
        val items = ChatTimelineProjector.project(
            messages = listOf(message("persisted-user", "user", "First")),
            pendingMessages = listOf(message("pending-local-1", "user", "Second")),
            activeTurn = activeMessage("turn-1", "Reply", "active", 2),
            hasMoreMessages = false,
            activeTurnAnchorId = "pending-local-1",
        )

        assertEquals("active-turn-1", items.single { it.type == ChatTimelineItemType.ASSISTANT_LIVE_TURN }.id)
    }

    @Test
    fun revisionChangesOnlyWhenProjectedContentChanges() {
        val tracker = ChatTimelineRevisionTracker()
        val first = ChatTimelineProjector.project(
            emptyList(),
            emptyList(),
            activeMessage("turn-1", "abc", "active", 1),
            false,
        )
        val changed = ChatTimelineProjector.project(
            emptyList(),
            emptyList(),
            activeMessage("turn-1", "abcdef", "active", 2),
            false,
        )

        assertEquals(1, tracker.update(first))
        assertEquals(1, tracker.update(first))
        assertEquals(2, tracker.update(changed))
        assertEquals(3, tracker.reset())
    }

    private fun message(
        id: String,
        role: String,
        text: String,
        status: String = if (role == "assistant") "done" else "sent",
        renderVersion: Int = 0,
        turnId: String? = null,
        detail: String? = null,
        timestamp: String? = null,
        thinking: String? = null,
        images: List<ImageAttachment>? = null,
    ): ChatMessage = ChatMessage(
        id = id,
        role = role,
        text = text,
        status = status,
        renderVersion = renderVersion,
        turnId = turnId,
        detail = detail,
        timestamp = timestamp,
        thinking = thinking,
        tools = null,
        items = null,
        images = images,
    )

    private fun activeMessage(
        turnId: String,
        text: String,
        status: String,
        renderVersion: Int,
    ): ChatMessage = message(
        id = "active-$turnId",
        role = "assistant",
        text = text,
        status = status,
        renderVersion = renderVersion,
        turnId = turnId,
    )

    private fun messageWithImage(
        id: String,
        name: String,
        dataUrl: String,
    ): ChatMessage = message(
        id = id,
        role = "user",
        text = "Analyze image",
        images = listOf(ImageAttachment(name = name, dataUrl = dataUrl)),
    )
}
