package com.bitfun.mobile.core.feature.session

import com.bitfun.mobile.core.protocol.ActiveTurnSnapshotResponse
import com.bitfun.mobile.core.protocol.ChatMessageItemResponse
import com.bitfun.mobile.core.protocol.ChatMessageResponse
import com.bitfun.mobile.core.protocol.RemoteToolStatusResponse
import com.bitfun.mobile.core.protocol.SessionItemResponse
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class RemoteResponseMapperTest {
    @Test
    fun mapsAssistantMessagesAndNestedTools() {
        val message = RemoteResponseMapper.chatMessage(
            ChatMessageResponse(
                id = "m1",
                role = "assistant",
                content = "",
                thinking = "Thinking out loud",
                timestamp = "2026-06-10T12:00:00.000Z",
                items = listOf(
                    ChatMessageItemResponse(
                        type = "tool",
                        tool = RemoteToolStatusResponse(id = "tool-1", name = "shell", status = "pending"),
                        subItems = listOf(
                            ChatMessageItemResponse(
                                type = "tool",
                                tool = RemoteToolStatusResponse(id = "tool-2", name = "edit", status = "done"),
                            ),
                        ),
                    ),
                ),
            ),
        )

        assertEquals("m1", message.id)
        assertEquals("", message.text)
        assertEquals("Thinking out loud", message.thinking)
        assertEquals("done", message.status)
        assertEquals("shell · pending\nedit · done", message.detail)
        assertEquals(2, message.tools?.size)
    }

    @Test
    fun mapsActiveTurnWithItemFallbackTextAndExplicitTools() {
        val message = RemoteResponseMapper.activeTurn(
            ActiveTurnSnapshotResponse(
                turnId = "turn-1",
                status = "active",
                items = listOf(ChatMessageItemResponse(type = "text", content = "Working on it")),
                tools = listOf(RemoteToolStatusResponse(id = "tool-3", name = "read_file", status = "running")),
            ),
        )

        assertEquals("active-turn-1", message.id)
        assertEquals("turn-1", message.turnId)
        assertEquals("Working on it", message.text)
        assertEquals("read_file · running", message.detail)
    }

    @Test
    fun doesNotPromoteSubagentProgressToActiveText() {
        val message = RemoteResponseMapper.activeTurn(
            ActiveTurnSnapshotResponse(
                turnId = "turn-structured",
                status = "active",
                items = listOf(
                    ChatMessageItemResponse(
                        type = "subagent",
                        isSubagent = true,
                        content = "Subagent check",
                        subItems = listOf(
                            ChatMessageItemResponse(type = "text", content = "Nested progress"),
                            ChatMessageItemResponse(
                                type = "tool",
                                tool = RemoteToolStatusResponse(id = "tool-nested-1", name = "inspect", status = "running"),
                            ),
                        ),
                    ),
                ),
            ),
        )

        assertEquals("", message.text)
        assertEquals(1, message.tools?.size)
        assertTrue(message.detail?.contains("inspect") == true)
    }

    @Test
    fun mapsAlternateSessionFieldsAndSafeDefaults() {
        val session = RemoteResponseMapper.session(
            SessionItemResponse(
                id = "session-1",
                title = null,
                agentType = null,
                status = null,
                updatedAt = "1781092800000",
                createdAt = "2026-06-09 12:00:00",
                messageCount = null,
                workspacePath = null,
                workspaceName = null,
            ),
        )

        assertEquals("session-1", session.id)
        assertEquals("Session sessio", session.title)
        assertEquals("code", session.agentType)
        assertEquals("idle", session.status)
        assertEquals("1781092800000", session.updatedAt)
        assertEquals(0, session.messageCount)
    }
}
