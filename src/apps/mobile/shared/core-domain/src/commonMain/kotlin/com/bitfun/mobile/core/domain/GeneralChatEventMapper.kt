package com.bitfun.mobile.core.domain

import com.bitfun.mobile.core.protocol.GeneralChatApiTool
import com.bitfun.mobile.core.protocol.GeneralChatStreamEvent
import com.bitfun.mobile.core.protocol.RemoteToolStatusResponse

public data class GeneralChatProjectionItem public constructor(
    public val id: String,
    public val type: String,
    public val content: String,
    public val tool: RemoteToolStatusResponse?,
)

public data class GeneralChatTurnProjection public constructor(
    public val sessionId: String,
    public val turnId: String,
    public val version: Int,
    public val text: String,
    public val thinking: String,
    public val status: String,
    public val errorCode: String,
    public val capabilityRequired: String,
    public val tools: List<RemoteToolStatusResponse>,
    public val items: List<GeneralChatProjectionItem>,
)

public class GeneralChatEventException public constructor(message: String) : IllegalStateException(message)

public object GeneralChatEventMapper {
    public fun emptyProjection(sessionId: String, turnId: String): GeneralChatTurnProjection =
        emptyProjection(sessionId, turnId, 0)

    public fun emptyProjection(sessionId: String, turnId: String, version: Int): GeneralChatTurnProjection =
        GeneralChatTurnProjection(sessionId, turnId, version, "", "", "starting", "", "", emptyList(), emptyList())

    public fun apply(
        current: GeneralChatTurnProjection,
        event: GeneralChatStreamEvent,
    ): GeneralChatTurnProjection {
        if (event.sessionId != current.sessionId || event.turnId != current.turnId) {
            throw GeneralChatEventException("General chat event does not belong to the active turn.")
        }
        if (event.version <= current.version || current.status in TERMINAL_STATUSES) return current
        if (event.version != current.version + 1) {
            throw GeneralChatEventException("General chat event version gap.")
        }
        var next = current.copy(version = event.version)
        when (event.type) {
            "turn.started" -> next = next.copy(status = "starting")
            "message.delta" -> next = next.copy(
                text = next.text + event.delta.orEmpty(),
                items = appendTextItem(next.items, event.eventId, "text", event.delta.orEmpty()),
                status = "streaming",
            )
            "thinking.delta" -> next = next.copy(
                thinking = next.thinking + event.delta.orEmpty(),
                items = appendTextItem(next.items, event.eventId, "thinking", event.delta.orEmpty()),
                status = "thinking",
            )
            "tool.started", "tool.updated", "tool.completed" -> {
                event.tool?.let { tool ->
                    val mapped = mapTool(tool, event.type)
                    next = next.copy(
                        tools = upsertTool(next.tools, mapped),
                        items = upsertToolItem(next.items, mapped, event.eventId),
                    )
                }
                next = next.copy(status = "tool_calling")
            }
            "permission.required" -> {
                event.tool?.let { tool ->
                    val mapped = mapTool(tool, event.type)
                    next = next.copy(
                        tools = upsertTool(next.tools, mapped),
                        items = upsertToolItem(next.items, mapped, event.eventId),
                    )
                }
                next = next.copy(status = "pending_confirmation")
            }
            "capability.required" -> next = next.copy(
                capabilityRequired = event.capability ?: "unknown",
                status = "capability_required",
            )
            "turn.finishing" -> next = next.copy(status = "finishing")
            "turn.completed" -> next = next.copy(status = "completed")
            "turn.cancelled" -> next = next.copy(status = "cancelled")
            "turn.failed" -> next = next.copy(status = "failed", errorCode = event.errorCode ?: "unknown")
        }
        return next
    }

    private fun appendTextItem(
        items: List<GeneralChatProjectionItem>,
        eventId: String,
        type: String,
        delta: String,
    ): List<GeneralChatProjectionItem> {
        if (delta.isEmpty()) return items
        val last = items.lastOrNull()
        if (last != null && last.type == type && last.tool == null) {
            return items.dropLast(1) + last.copy(content = last.content + delta)
        }
        return items + GeneralChatProjectionItem(eventId, type, delta, null)
    }

    private fun upsertToolItem(
        items: List<GeneralChatProjectionItem>,
        incoming: RemoteToolStatusResponse,
        eventId: String,
    ): List<GeneralChatProjectionItem> {
        val index = items.indexOfFirst { it.tool?.id == incoming.id }
        val mapped = GeneralChatProjectionItem(incoming.id?.takeIf(String::isNotEmpty) ?: eventId, "tool", "", incoming)
        if (index < 0) return items + mapped
        return items.toMutableList().also { it[index] = mapped }
    }

    private fun upsertTool(
        tools: List<RemoteToolStatusResponse>,
        incoming: RemoteToolStatusResponse,
    ): List<RemoteToolStatusResponse> {
        val index = tools.indexOfFirst { it.id == incoming.id }
        if (index < 0) return tools + incoming
        return tools.toMutableList().also { it[index] = incoming }
    }

    private fun mapTool(incoming: GeneralChatApiTool, eventType: String): RemoteToolStatusResponse {
        val status = when {
            eventType == "permission.required" -> "pending_confirmation"
            incoming.status.isNotEmpty() -> incoming.status
            eventType == "tool.started" -> "preparing"
            eventType == "tool.completed" -> "completed"
            else -> incoming.status
        }
        return RemoteToolStatusResponse(
            id = incoming.id,
            name = incoming.name,
            status = status,
            inputPreview = incoming.inputPreview,
            resultPreview = incoming.resultPreview,
            errorPreview = incoming.errorPreview,
        )
    }

    private val TERMINAL_STATUSES = setOf("completed", "cancelled", "failed")
}
