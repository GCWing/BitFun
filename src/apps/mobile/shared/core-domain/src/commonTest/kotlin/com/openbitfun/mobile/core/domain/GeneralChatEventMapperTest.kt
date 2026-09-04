package com.openbitfun.mobile.core.domain

import com.openbitfun.mobile.core.protocol.GeneralChatApiTool
import com.openbitfun.mobile.core.protocol.GeneralChatStreamEvent
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class GeneralChatEventMapperTest {
    @Test
    fun projectsOrderedDeltasAndIgnoresDuplicateVersions() {
        var projection = GeneralChatEventMapper.emptyProjection("chat-1", "turn-1")
        projection = GeneralChatEventMapper.apply(projection, event(1, "message.delta", delta = "hello"))
        projection = GeneralChatEventMapper.apply(projection, event(1, "message.delta", delta = "duplicate"))
        projection = GeneralChatEventMapper.apply(projection, event(2, "turn.completed"))

        assertEquals("hello", projection.text)
        assertEquals("completed", projection.status)
        assertEquals(2, projection.version)
    }

    @Test
    fun rejectsVersionGaps() {
        val projection = GeneralChatEventMapper.emptyProjection("chat-1", "turn-1", 3)
        assertFailsWith<GeneralChatEventException> {
            GeneralChatEventMapper.apply(projection, event(5, "message.delta", delta = "gap"))
        }
    }

    @Test
    fun preservesTextAndToolOrderWhileUpdatingStableToolItem() {
        var projection = GeneralChatEventMapper.emptyProjection("chat-1", "turn-1")
        projection = GeneralChatEventMapper.apply(projection, event(1, "message.delta", "text-1", "before"))
        projection = GeneralChatEventMapper.apply(
            projection,
            event(2, "tool.started", "tool-event-1", tool = GeneralChatApiTool("tool-1", "WebSearch", "running")),
        )
        projection = GeneralChatEventMapper.apply(
            projection,
            event(3, "tool.completed", "tool-event-2", tool = GeneralChatApiTool("tool-1", "WebSearch", "completed")),
        )
        projection = GeneralChatEventMapper.apply(projection, event(4, "message.delta", "text-2", "after"))

        assertEquals(3, projection.items.size)
        assertEquals("before", projection.items[0].content)
        assertEquals("tool-1", projection.items[1].tool?.id)
        assertEquals("completed", projection.items[1].tool?.status)
        assertEquals("after", projection.items[2].content)
        assertEquals(1, projection.tools.size)
    }

    @Test
    fun projectsPermissionCapabilityAndFreezesTerminalState() {
        var projection = GeneralChatEventMapper.emptyProjection("chat-1", "turn-1")
        projection = GeneralChatEventMapper.apply(
            projection,
            event(1, "permission.required", tool = GeneralChatApiTool("tool-1", "Write", "waiting")),
        )
        assertEquals("pending_confirmation", projection.status)
        assertEquals("pending_confirmation", projection.tools[0].status)

        projection = GeneralChatEventMapper.apply(
            projection,
            event(2, "capability.required", capability = "remote_workspace"),
        )
        projection = GeneralChatEventMapper.apply(projection, event(3, "turn.cancelled"))
        projection = GeneralChatEventMapper.apply(projection, event(4, "message.delta", delta = "late"))

        assertEquals("remote_workspace", projection.capabilityRequired)
        assertEquals("cancelled", projection.status)
        assertEquals(3, projection.version)
        assertEquals("", projection.text)
    }

    private fun event(
        version: Int,
        type: String,
        eventId: String = "event-$version",
        delta: String? = null,
        tool: GeneralChatApiTool? = null,
        capability: String? = null,
    ): GeneralChatStreamEvent = GeneralChatStreamEvent(
        eventId = eventId,
        version = version,
        sessionId = "chat-1",
        turnId = "turn-1",
        type = type,
        delta = delta,
        tool = tool,
        errorCode = null,
        capability = capability,
    )
}
