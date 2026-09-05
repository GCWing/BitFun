package com.openbitfun.mobile.core.domain

import com.openbitfun.mobile.core.protocol.RemoteToolStatusResponse

public sealed interface ConversationEvent {
    public data class UserMessage public constructor(
        public val message: ChatMessage,
        public val persisted: Boolean,
    ) : ConversationEvent

    public data class AssistantDelta public constructor(
        public val sessionId: String,
        public val turnId: String,
        public val delta: String,
    ) : ConversationEvent

    public data class AssistantMessage public constructor(
        public val message: ChatMessage,
    ) : ConversationEvent

    public data class ActiveTurnUpdated public constructor(
        public val sessionId: String,
        public val message: ChatMessage,
    ) : ConversationEvent

    public data class ToolStarted public constructor(
        public val sessionId: String,
        public val turnId: String,
        public val tool: RemoteToolStatusResponse,
    ) : ConversationEvent

    public data class ToolFinished public constructor(
        public val sessionId: String,
        public val turnId: String,
        public val tool: RemoteToolStatusResponse,
    ) : ConversationEvent

    public data class TurnStarted public constructor(
        public val sessionId: String,
        public val turnId: String,
    ) : ConversationEvent

    public data class TurnFinished public constructor(
        public val sessionId: String,
        public val turnId: String,
        public val message: ChatMessage?,
    ) : ConversationEvent

    public data class SessionUpdated public constructor(
        public val session: SessionSummary,
    ) : ConversationEvent

    public data class Error public constructor(
        public val message: String,
    ) : ConversationEvent
}
