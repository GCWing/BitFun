package com.bitfun.mobile.core.feature.session

/** Which list a session row is being shown in. */
public enum class SessionActionScope {
    /** The on-device general chat list, backed by local storage. */
    GENERAL,

    /** A paired desktop's sessions, where every action is a round trip. */
    REMOTE,
}

public data class SessionActionCapabilities public constructor(
    public val canViewDetails: Boolean,
    public val canArchive: Boolean,
    public val canExport: Boolean,
    public val canDelete: Boolean,
)

/**
 * What a session row's overflow menu may offer.
 *
 * Ported from `pages/policy/SessionActionPolicy.ets`. Archive and export are
 * local-storage operations, so a remote session never offers them however it is
 * typed; while a command is in flight nothing is offered at all, because the
 * row the menu was opened over may not survive the answer.
 */
public object SessionActionPolicy {
    public fun resolve(
        scope: SessionActionScope,
        agentType: String,
        busy: Boolean,
    ): SessionActionCapabilities {
        if (busy) return SessionActionCapabilities(false, false, false, false)
        if (scope == SessionActionScope.REMOTE) {
            return SessionActionCapabilities(true, false, false, true)
        }
        // Two names for one kind: the desktop and the HarmonyOS client say
        // "chat", the on-device store writes "general_chat" into its own rows.
        val isGeneralChat = agentType.lowercase() in GENERAL_CHAT_AGENT_TYPES
        return SessionActionCapabilities(true, isGeneralChat, isGeneralChat, true)
    }

    private val GENERAL_CHAT_AGENT_TYPES = setOf("chat", "general_chat")
}
