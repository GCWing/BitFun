package com.bitfun.mobile.core.domain

import com.bitfun.mobile.core.protocol.ChatMessageItemResponse
import kotlin.math.absoluteValue

/**
 * Harmony-parity revision tracking port, currently not consumed by the Kotlin UI.
 * Android Compose uses `items(key = ...)` and iOS uses `ForEach` ids, so this
 * tracker intentionally keeps coarse signatures only.
 */
public class ChatTimelineRevisionTracker public constructor() {
    private var signature: String = ""
    private var revision: Int = 0

    public fun update(items: List<ChatTimelineItem>): Int {
        val nextSignature = items.joinToString("|") { item ->
            val message = item.message
            listOf(
                item.type.name,
                item.id,
                item.isStreaming,
                item.isFinalizing,
                message?.status.orEmpty(),
                message?.renderVersion ?: 0,
                message?.let { textSignature(it.text) }.orEmpty(),
                message?.thinking?.let(::textSignature).orEmpty(),
                message?.items?.size ?: 0,
                message?.tools?.size ?: 0,
            ).joinToString(":")
        }
        if (nextSignature != signature) {
            signature = nextSignature
            revision += 1
        }
        return revision
    }

    public fun reset(): Int {
        signature = ""
        revision += 1
        return revision
    }

    private fun textSignature(value: String): String =
        "${value.length}:${stableTextHash(value)}"
}

public object ChatTimelineProjector {
    public fun project(
        messages: List<ChatMessage>,
        pendingMessages: List<ChatMessage>,
        activeTurn: ChatMessage?,
        hasMoreMessages: Boolean,
    ): List<ChatTimelineItem> = project(messages, pendingMessages, activeTurn, hasMoreMessages, "")

    public fun project(
        messages: List<ChatMessage>,
        pendingMessages: List<ChatMessage>,
        activeTurn: ChatMessage?,
        hasMoreMessages: Boolean,
        activeTurnAnchorId: String,
    ): List<ChatTimelineItem> {
        val timelineMessages = realMessages(messages)
        val pendingItems = pendingMessagesNotPersisted(pendingMessages, timelineMessages)
        val renderActiveTurn = activeTurn?.takeIf { shouldRenderActiveTurn(timelineMessages, it) }
        val anchorIndex = anchorIndex(pendingItems, activeTurnAnchorId)
        val items = timelineMessages.map { message ->
            ChatTimelineItem(
                id = "message-${message.id}",
                type = messageItemType(message),
                message = message,
                isStreaming = false,
                isFinalizing = false,
                showRetryAction = false,
            )
        }.toMutableList()

        if (renderActiveTurn != null && anchorIndex < 0) items += activeTurnItem(renderActiveTurn)
        pendingItems.forEachIndexed { index, message ->
            items += ChatTimelineItem(
                id = "pending-${message.id}",
                type = ChatTimelineItemType.OPTIMISTIC_USER_MESSAGE,
                message = message,
                isStreaming = false,
                isFinalizing = false,
                showRetryAction = false,
            )
            if (renderActiveTurn != null && index == anchorIndex) items += activeTurnItem(renderActiveTurn)
        }

        if (items.isEmpty() && !hasMoreMessages) {
            items += ChatTimelineItem(
                id = "empty-state",
                type = ChatTimelineItemType.EMPTY_STATE,
                message = null,
                isStreaming = false,
                isFinalizing = false,
                showRetryAction = false,
            )
        }

        markLatestFailedMessageRetryable(items)
        return items
    }

    private fun activeTurnItem(activeTurn: ChatMessage): ChatTimelineItem = ChatTimelineItem(
        id = "active-${activeTurnKey(activeTurn)}",
        type = ChatTimelineItemType.ASSISTANT_LIVE_TURN,
        message = activeTurn,
        isStreaming = MessageStatusSemantics.isStreaming(activeTurn.status),
        isFinalizing = MessageStatusSemantics.isFinalizing(activeTurn.status),
        showRetryAction = false,
    )

    private fun anchorIndex(pendingItems: List<ChatMessage>, id: String): Int =
        if (id.isEmpty()) -1 else pendingItems.indexOfFirst { it.id == id }

    public fun pendingMessagesNotPersisted(
        pendingMessages: List<ChatMessage>,
        messages: List<ChatMessage>,
    ): List<ChatMessage> = pendingMessages.filter { pending ->
        messages.none { message ->
            pending.role == "user" && message.role == "user" && pending.id == message.id
        }
    }

    private fun markLatestFailedMessageRetryable(items: MutableList<ChatTimelineItem>) {
        for (index in items.indices.reversed()) {
            val message = items[index].message ?: continue
            items[index] = items[index].copy(
                showRetryAction = MessageStatusSemantics.isRetryableFailure(message.status),
            )
            return
        }
    }

    private fun realMessages(messages: List<ChatMessage>): List<ChatMessage> =
        messages.filterNot(::isSeedMessage)

    private fun shouldRenderActiveTurn(
        messages: List<ChatMessage>,
        activeTurn: ChatMessage,
    ): Boolean {
        if (activeTurn.id.isEmpty()) return false
        return messages.none { message -> isPersistedAssistantDuplicate(activeTurn, message) }
    }

    private fun isSeedMessage(message: ChatMessage): Boolean =
        message.id.startsWith("system-") && message.role == "assistant"

    private fun messageItemType(message: ChatMessage): ChatTimelineItemType =
        if (message.role == "user") {
            ChatTimelineItemType.USER_MESSAGE
        } else {
            ChatTimelineItemType.ASSISTANT_MESSAGE
        }

    private fun isPersistedAssistantDuplicate(
        activeTurn: ChatMessage,
        message: ChatMessage,
    ): Boolean {
        if (message.role != "assistant" || !hasDisplayableAssistantFinal(message)) return false
        if (message.id == activeTurn.id) return true
        if (!activeTurn.turnId.isNullOrEmpty() && message.id == "${activeTurn.turnId}_assistant") {
            return true
        }
        return !activeTurn.turnId.isNullOrEmpty() && !message.turnId.isNullOrEmpty() &&
            activeTurn.turnId == message.turnId
    }

    private fun hasDisplayableAssistantFinal(message: ChatMessage): Boolean {
        val text = message.text.trim()
        if (text.isNotEmpty() && text != message.thinking.orEmpty().trim()) return true
        return lastTopLevelText(message.items.orEmpty()).isNotEmpty()
    }

    private fun lastTopLevelText(items: List<ChatMessageItemResponse>): String {
        for (item in items.asReversed()) {
            val type = item.type.orEmpty().lowercase()
            val content = item.content.orEmpty().trim()
            if (
                content.isNotEmpty() &&
                item.tool == null &&
                item.isSubagent != true &&
                type !in setOf("thinking", "tool", "subagent", "agent")
            ) {
                return content
            }
        }
        return ""
    }

    private fun activeTurnKey(activeTurn: ChatMessage): String =
        activeTurn.turnId?.takeIf(String::isNotEmpty) ?: activeTurn.id

}

private fun stableTextHash(text: String): String {
    var hash = 0
    text.forEach { character -> hash = (hash shl 5) - hash + character.code }
    return hash.toLong().absoluteValue.toString(36)
}
