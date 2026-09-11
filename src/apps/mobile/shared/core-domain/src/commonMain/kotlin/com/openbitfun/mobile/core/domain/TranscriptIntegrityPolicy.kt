package com.openbitfun.mobile.core.domain

import com.openbitfun.mobile.core.protocol.ChatMessageItemResponse

/** Guards cursor resume against cached assistant rows whose display body was lost. */
public object TranscriptIntegrityPolicy {
    public fun hasDisplayableBody(message: ChatMessage): Boolean =
        message.text.isNotBlank() ||
            !message.thinking.isNullOrBlank() ||
            !message.tools.isNullOrEmpty() ||
            hasRenderableItems(message.items) ||
            !message.images.isNullOrEmpty()

    public fun isHollowAssistant(message: ChatMessage): Boolean =
        message.role.equals("assistant", ignoreCase = true) && !hasDisplayableBody(message)

    public fun hasHollowAssistants(messages: List<ChatMessage>): Boolean =
        messages.any(::isHollowAssistant)

    private fun hasRenderableItems(items: List<ChatMessageItemResponse>?): Boolean =
        items.orEmpty().any { item ->
            !item.content.isNullOrBlank() || item.tool != null || hasRenderableItems(item.subItems)
        }
}
