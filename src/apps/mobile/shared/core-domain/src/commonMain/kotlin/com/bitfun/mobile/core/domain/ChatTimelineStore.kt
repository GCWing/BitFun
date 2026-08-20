package com.bitfun.mobile.core.domain

import com.bitfun.mobile.core.protocol.ChatMessageItemResponse
import com.bitfun.mobile.core.protocol.ImageAttachment
import com.bitfun.mobile.core.protocol.RemoteDefaultModels
import com.bitfun.mobile.core.protocol.RemoteModelCatalog
import com.bitfun.mobile.core.protocol.RemoteToolStatusResponse

public enum class ChatSyncPhase {
    IDLE,
    LOADING,
    SENDING,
    STREAMING,
    FINALIZING,
    RECONNECTING,
    ERROR,
}

public data class ChatTimelineState public constructor(
    public val sessionId: String,
    public val persistedMessages: List<ChatMessage>,
    public val optimisticMessages: List<ChatMessage>,
    public val activeTurn: ChatMessage?,
    public val syncPhase: ChatSyncPhase,
    public val cursor: ChatSessionCursor,
    public val modelCatalog: RemoteModelCatalog,
    public val selectedModelId: String,
)

public class ChatTimelineStore public constructor() {
    private var state: ChatTimelineState = emptyState("")

    public fun reset(): Unit = reset("")

    public fun reset(sessionId: String) {
        state = emptyState(sessionId)
    }

    public fun snapshot(): ChatTimelineState = state.copy(
        persistedMessages = state.persistedMessages.toList(),
        optimisticMessages = state.optimisticMessages.toList(),
        cursor = state.cursor.copy(),
    )

    public fun setSyncPhase(syncPhase: ChatSyncPhase) {
        state = state.copy(syncPhase = syncPhase)
    }

    public fun setCursor(cursor: ChatSessionCursor) {
        state = state.copy(cursor = cursor.copy())
    }

    public fun setModelCatalog(modelCatalog: RemoteModelCatalog, selectedModelId: String) {
        state = state.copy(
            cursor = state.cursor.copy(knownModelCatalogVersion = modelCatalog.version),
            modelCatalog = modelCatalog,
            selectedModelId = selectedModelId,
        )
    }

    public fun setSelectedModelId(selectedModelId: String) {
        state = state.copy(selectedModelId = selectedModelId)
    }

    public fun setPersistedMessages(messages: List<ChatMessage>) {
        val persisted = realMessages(messages)
        state = state.copy(
            persistedMessages = persisted,
            optimisticMessages = optimisticMessagesNotPersisted(state.optimisticMessages, persisted),
            activeTurn = state.activeTurn?.takeUnless { isActiveTurnCoveredByMessages(it, persisted) },
        )
    }

    public fun mergePersistedMessages(messages: List<ChatMessage>) {
        val persisted = mergeMessages(state.persistedMessages, realMessages(messages))
        state = state.copy(
            persistedMessages = persisted,
            optimisticMessages = optimisticMessagesNotPersisted(state.optimisticMessages, persisted),
            activeTurn = state.activeTurn?.takeUnless { isActiveTurnCoveredByMessages(it, persisted) },
        )
    }

    public fun appendOptimisticMessage(message: ChatMessage) {
        state = state.copy(
            optimisticMessages = state.optimisticMessages + message,
            syncPhase = ChatSyncPhase.SENDING,
        )
    }

    public fun markOptimisticMessageFailed(messageId: String) {
        state = state.copy(
            optimisticMessages = state.optimisticMessages.map { message ->
                if (message.id == messageId) message.copy(status = "failed") else message
            },
            syncPhase = ChatSyncPhase.ERROR,
        )
    }

    public fun setLocalActiveTurn(turnId: String) {
        val normalizedTurnId = turnId.trim()
        if (normalizedTurnId.isEmpty()) return
        val activeId = "active-$normalizedTurnId"
        val existing = state.activeTurn
        if (existing?.id == activeId) {
            val activeTurn = existing.copy(
                turnId = existing.turnId?.takeIf(String::isNotEmpty) ?: normalizedTurnId,
                status = existing.status.ifEmpty { "active" },
            )
            state = state.copy(
                activeTurn = activeTurn,
                syncPhase = phaseForActiveTurn(activeTurn, ChatSyncPhase.STREAMING),
            )
            return
        }
        if (existing != null && existing.id.isNotEmpty() && existing.id != activeId &&
            !isLocalPendingActiveTurn(existing)
        ) {
            return
        }
        state = state.copy(
            activeTurn = emptyMessage(
                id = activeId,
                turnId = normalizedTurnId,
                status = "active",
            ),
            syncPhase = ChatSyncPhase.STREAMING,
        )
    }

    public fun setPendingActiveTurn(localId: String): String {
        val normalizedLocalId = localId.trim()
        if (normalizedLocalId.isEmpty()) return ""
        val activeId = "active-pending-$normalizedLocalId"
        val existing = state.activeTurn
        if (existing != null && existing.id.isNotEmpty() && existing.id != activeId &&
            !isLocalPendingActiveTurn(existing)
        ) {
            return ""
        }
        state = state.copy(
            activeTurn = emptyMessage(id = activeId, turnId = null, status = "active"),
            syncPhase = ChatSyncPhase.STREAMING,
        )
        return activeId
    }

    public fun clearPendingActiveTurn(activeId: String) {
        val activeTurn = state.activeTurn
        if (activeTurn == null || activeTurn.id != activeId || !isLocalPendingActiveTurn(activeTurn)) return
        state = state.copy(activeTurn = null, syncPhase = ChatSyncPhase.IDLE)
    }

    public fun setActiveTurn(activeTurn: ChatMessage?) {
        val normalized = activeTurn?.takeIf { it.id.isNotEmpty() }
        val merged = activeTurnForUpdate(state.activeTurn, normalized)
        val next = activeTurnForState(merged)
        state = state.copy(
            activeTurn = next,
            syncPhase = phaseForActiveTurn(next, state.syncPhase),
        )
    }

    public fun clearActiveTurn() {
        state = state.copy(activeTurn = null, syncPhase = ChatSyncPhase.IDLE)
    }

    public fun applySnapshot(snapshot: ChatSessionSnapshot) {
        setCursor(snapshot.cursor)
        if (snapshot.newMessages.isNotEmpty()) mergePersistedMessages(snapshot.newMessages)
        setActiveTurn(snapshot.activeTurn)
        snapshot.modelCatalog?.let { catalog ->
            setModelCatalog(catalog, selectedModelIdForCatalog(catalog, state.selectedModelId))
        }
    }

    public fun applyEvent(event: ConversationEvent) {
        when (event) {
            is ConversationEvent.UserMessage -> {
                if (event.persisted) mergePersistedMessages(listOf(event.message))
                else appendOptimisticMessage(event.message)
            }
            is ConversationEvent.TurnStarted -> {
                if (acceptsSession(event.sessionId)) setLocalActiveTurn(event.turnId)
            }
            is ConversationEvent.AssistantDelta -> {
                if (acceptsSession(event.sessionId)) appendAssistantDelta(event.turnId, event.delta)
            }
            is ConversationEvent.AssistantMessage -> {
                mergePersistedMessages(listOf(event.message))
                clearActiveTurn()
            }
            is ConversationEvent.ActiveTurnUpdated -> {
                if (acceptsSession(event.sessionId)) setActiveTurn(event.message)
            }
            is ConversationEvent.ToolStarted -> {
                if (acceptsSession(event.sessionId)) updateActiveTool(event.turnId, event.tool)
            }
            is ConversationEvent.ToolFinished -> {
                if (acceptsSession(event.sessionId)) updateActiveTool(event.turnId, event.tool)
            }
            is ConversationEvent.TurnFinished -> {
                val message = event.message
                if (message != null) {
                    mergePersistedMessages(listOf(message))
                    clearActiveTurn()
                } else if (acceptsSession(event.sessionId)) {
                    val active = state.activeTurn
                    if (active?.turnId == event.turnId) setActiveTurn(active.copy(status = "completed"))
                }
            }
            is ConversationEvent.SessionUpdated -> Unit
            is ConversationEvent.Error -> setSyncPhase(ChatSyncPhase.ERROR)
        }
    }

    public fun activeTurnOrNull(): ChatMessage? = state.activeTurn

    public fun project(hasMoreMessages: Boolean): List<ChatTimelineItem> =
        ChatTimelineProjector.project(
            state.persistedMessages,
            state.optimisticMessages,
            state.activeTurn,
            hasMoreMessages,
        )

    private fun acceptsSession(sessionId: String): Boolean =
        sessionId.isEmpty() || state.sessionId.isEmpty() || state.sessionId == sessionId

    private fun appendAssistantDelta(turnId: String, delta: String) {
        if (delta.isEmpty()) return
        if (state.activeTurn?.turnId != turnId) setLocalActiveTurn(turnId)
        val active = state.activeTurn ?: return
        setActiveTurn(
            active.copy(
                text = active.text + delta,
                status = "active",
                renderVersion = (active.renderVersion ?: 0) + delta.length,
            ),
        )
    }

    private fun updateActiveTool(turnId: String, tool: RemoteToolStatusResponse) {
        if (state.activeTurn?.turnId != turnId) setLocalActiveTurn(turnId)
        val active = state.activeTurn ?: return
        val tools = active.tools.orEmpty().filter { it.id != tool.id } + tool
        setActiveTurn(
            active.copy(
                status = "active",
                tools = tools,
                renderVersion = (active.renderVersion ?: 0) + 1,
            ),
        )
    }

    private fun activeTurnForState(activeTurn: ChatMessage?): ChatMessage? {
        if (activeTurn != null && activeTurn.id.isNotEmpty()) {
            return activeTurn.takeUnless { isActiveTurnCoveredByMessages(it, state.persistedMessages) }
        }
        val previous = state.activeTurn
        if (previous != null && MessageStatusSemantics.shouldHoldCompletedTurn(previous.status) &&
            hasDisplayableAssistantFinal(previous) &&
            !isActiveTurnCoveredByMessages(previous, state.persistedMessages)
        ) {
            return previous
        }
        return null
    }

    public companion object {
        public fun optimisticMessagesNotPersisted(
            optimisticMessages: List<ChatMessage>,
            persistedMessages: List<ChatMessage>,
        ): List<ChatMessage> = optimisticMessages.filter { pending ->
            persistedMessages.none { message -> isPersistedUserDuplicate(pending, message) }
        }

        public fun isActiveTurnCoveredByMessages(
            activeTurn: ChatMessage,
            messages: List<ChatMessage>,
        ): Boolean = activeTurn.id.isEmpty() || messages.any { message ->
            isPersistedAssistantDuplicate(activeTurn, message)
        }

        private fun emptyState(sessionId: String): ChatTimelineState = ChatTimelineState(
            sessionId = sessionId,
            persistedMessages = emptyList(),
            optimisticMessages = emptyList(),
            activeTurn = null,
            syncPhase = ChatSyncPhase.IDLE,
            cursor = ChatSessionCursor(0, 0, 0),
            modelCatalog = RemoteModelCatalog(0, emptyList(), RemoteDefaultModels(), null),
            selectedModelId = "",
        )

        private fun emptyMessage(id: String, turnId: String?, status: String): ChatMessage = ChatMessage(
            id = id,
            role = "assistant",
            text = "",
            status = status,
            renderVersion = null,
            turnId = turnId,
            detail = "",
            timestamp = null,
            thinking = null,
            tools = null,
            items = null,
            images = null,
        )

        private fun realMessages(messages: List<ChatMessage>): List<ChatMessage> =
            messages.filterNot { it.id.startsWith("system-") && it.role == "assistant" }

        private fun phaseForActiveTurn(activeTurn: ChatMessage?, fallback: ChatSyncPhase): ChatSyncPhase {
            if (activeTurn == null || activeTurn.id.isEmpty()) {
                return if (fallback == ChatSyncPhase.SENDING) fallback else ChatSyncPhase.IDLE
            }
            if (MessageStatusSemantics.isStreaming(activeTurn.status)) return ChatSyncPhase.STREAMING
            if (MessageStatusSemantics.isFinalizing(activeTurn.status)) return ChatSyncPhase.FINALIZING
            return fallback
        }

        private fun activeTurnForUpdate(previous: ChatMessage?, incoming: ChatMessage?): ChatMessage? {
            if (incoming == null || incoming.id.isEmpty()) return null
            return if (previous != null && previous.id.isNotEmpty() && sameActiveTurn(previous, incoming)) {
                mergeActiveTurn(previous, incoming)
            } else {
                incoming
            }
        }

        private fun sameActiveTurn(previous: ChatMessage, incoming: ChatMessage): Boolean {
            if (!previous.turnId.isNullOrEmpty() && previous.turnId == incoming.turnId) return true
            return previous.id == incoming.id && previous.id.startsWith("active-") && incoming.id.startsWith("active-")
        }

        private fun mergeActiveTurn(previous: ChatMessage, incoming: ChatMessage): ChatMessage = incoming.copy(
            turnId = incoming.turnId ?: previous.turnId,
            role = incoming.role.ifEmpty { previous.role },
            text = monotonicText(previous.text, incoming.text),
            status = incoming.status.ifEmpty { previous.status },
            timestamp = incoming.timestamp ?: previous.timestamp,
            thinking = monotonicText(previous.thinking.orEmpty(), incoming.thinking.orEmpty()).ifEmpty { null },
            tools = incoming.tools ?: previous.tools,
            items = mergeActiveItems(previous.items.orEmpty(), incoming.items.orEmpty()),
            images = incoming.images?.takeIf(List<ImageAttachment>::isNotEmpty) ?: previous.images,
        )

        private fun mergeActiveItems(
            previousItems: List<ChatMessageItemResponse>,
            incomingItems: List<ChatMessageItemResponse>,
        ): List<ChatMessageItemResponse> {
            if (incomingItems.isEmpty()) return previousItems
            val merged = previousItems.toMutableList()
            val matched = mutableSetOf<Int>()
            incomingItems.forEachIndexed { incomingIndex, incoming ->
                val toolId = incoming.tool?.id.orEmpty()
                val matchIndex = if (toolId.isNotEmpty()) {
                    previousItems.indices.firstOrNull { index ->
                        index !in matched && previousItems[index].tool?.id == toolId
                    } ?: -1
                } else if (
                    incomingIndex < previousItems.size && incomingIndex !in matched &&
                    sameActiveItem(previousItems[incomingIndex], incoming)
                ) {
                    incomingIndex
                } else {
                    -1
                }
                if (matchIndex >= 0) {
                    merged[matchIndex] = mergeActiveItem(previousItems[matchIndex], incoming)
                    matched += matchIndex
                } else {
                    merged += incoming
                }
            }
            return merged
        }

        private fun mergeActiveItem(
            previous: ChatMessageItemResponse,
            incoming: ChatMessageItemResponse,
        ): ChatMessageItemResponse {
            val content = if (isTextLikeItem(previous) && isTextLikeItem(incoming)) {
                monotonicText(previous.content.orEmpty(), incoming.content.orEmpty())
            } else {
                incoming.content ?: previous.content
            }
            return ChatMessageItemResponse(
                type = incoming.type ?: previous.type,
                content = content,
                tool = mergeTool(previous.tool, incoming.tool),
                isSubagent = incoming.isSubagent ?: previous.isSubagent,
                subItems = mergeActiveItems(previous.subItems.orEmpty(), incoming.subItems.orEmpty()),
            )
        }

        private fun sameActiveItem(previous: ChatMessageItemResponse, incoming: ChatMessageItemResponse): Boolean {
            if (previous.type.orEmpty().lowercase() != incoming.type.orEmpty().lowercase()) return false
            val previousToolId = previous.tool?.id.orEmpty()
            val incomingToolId = incoming.tool?.id.orEmpty()
            return if (previousToolId.isNotEmpty() || incomingToolId.isNotEmpty()) {
                previousToolId == incomingToolId
            } else {
                true
            }
        }

        private fun isTextLikeItem(item: ChatMessageItemResponse): Boolean =
            item.type.orEmpty().lowercase() in setOf("text", "message", "thinking")

        private fun monotonicText(previous: String, incoming: String): String = when {
            incoming.isEmpty() -> previous
            incoming.startsWith(previous) -> incoming
            previous.startsWith(incoming) -> previous
            incoming.length > previous.length && previous.isEmpty() -> incoming
            else -> previous
        }

        private fun mergeTool(
            previous: RemoteToolStatusResponse?,
            incoming: RemoteToolStatusResponse?,
        ): RemoteToolStatusResponse? = when {
            incoming == null -> previous
            previous == null -> incoming
            ToolStatusSemantics.shouldKeepPrevious(previous.status, incoming.status) -> previous
            else -> incoming
        }

        private fun isLocalPendingActiveTurn(activeTurn: ChatMessage): Boolean =
            activeTurn.id.startsWith("active-pending-") && activeTurn.turnId.isNullOrEmpty()

        private fun mergeMessages(
            current: List<ChatMessage>,
            incoming: List<ChatMessage>,
        ): List<ChatMessage> {
            val merged = current.toMutableList()
            incoming.forEach { message ->
                val existingIndex = merged.indexOfFirst { it.id == message.id }
                if (existingIndex >= 0) {
                    merged[existingIndex] = mergeMessageSnapshot(merged[existingIndex], message)
                } else {
                    val optimisticIndex = merged.indexOfFirst { isOptimisticDuplicate(it, message) }
                    if (optimisticIndex >= 0) merged[optimisticIndex] = message else merged += message
                }
            }
            return merged
        }

        private fun mergeMessageSnapshot(previous: ChatMessage, incoming: ChatMessage): ChatMessage {
            val hasIncomingText = incoming.text.trim().isNotEmpty()
            return incoming.copy(
                turnId = incoming.turnId ?: previous.turnId,
                role = incoming.role.ifEmpty { previous.role },
                text = if (hasIncomingText) incoming.text else previous.text,
                status = incoming.status.ifEmpty { previous.status },
                detail = incoming.detail ?: previous.detail,
                timestamp = incoming.timestamp ?: previous.timestamp,
                thinking = incoming.thinking ?: if (hasIncomingText) null else previous.thinking,
                tools = incoming.tools ?: previous.tools,
                items = incoming.items ?: previous.items,
                images = incoming.images ?: previous.images,
                renderVersion = incoming.renderVersion ?: previous.renderVersion,
            )
        }

        private fun isOptimisticDuplicate(local: ChatMessage, remote: ChatMessage): Boolean =
            local.role == "user" && local.id.startsWith("msg-") && local.role == remote.role &&
                local.text.trim().isNotEmpty() && local.text.trim() == remote.text.trim()

        private fun isPersistedAssistantDuplicate(activeTurn: ChatMessage, message: ChatMessage): Boolean {
            if (message.role != "assistant" || !hasDisplayableAssistantFinal(message)) return false
            if (message.id == activeTurn.id) return true
            if (!activeTurn.turnId.isNullOrEmpty() && message.id == "${activeTurn.turnId}_assistant") return true
            return activeTurn.text.trim().isNotEmpty() && activeTurn.text.trim() == message.text.trim()
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
                if (content.isNotEmpty() && item.tool == null && item.isSubagent != true &&
                    type !in setOf("thinking", "tool", "subagent", "agent")
                ) {
                    return content
                }
            }
            return ""
        }

        private fun isPersistedUserDuplicate(pending: ChatMessage, message: ChatMessage): Boolean {
            if (pending.role != "user" || message.role != "user") return false
            if (pending.id == message.id) return true
            val pendingText = pending.text.trim()
            if (pendingText.isEmpty() || pendingText != message.text.trim()) return false
            return imageSignature(pending.images.orEmpty()) == imageSignature(message.images.orEmpty())
        }

        private fun imageSignature(images: List<ImageAttachment>): String =
            images.map { "${it.name}:${it.dataUrl}" }.sorted().joinToString("|")

        private fun selectedModelIdForCatalog(catalog: RemoteModelCatalog, current: String): String {
            catalog.sessionModelId?.takeIf(String::isNotEmpty)?.let { return it }
            if (current.isNotEmpty() && catalog.models.any { it.id == current && it.enabled }) return current
            return catalog.defaultModels.primary ?: catalog.defaultModels.fast ?: current
        }
    }
}
