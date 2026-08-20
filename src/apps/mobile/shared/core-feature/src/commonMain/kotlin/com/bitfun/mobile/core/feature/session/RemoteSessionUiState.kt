package com.bitfun.mobile.core.feature.session

import com.bitfun.mobile.core.domain.ChatTimelineState
import com.bitfun.mobile.core.domain.RemoteSession
import com.bitfun.mobile.core.domain.SessionAgentTypes

/**
 * Which agent kinds the session list is narrowed to.
 *
 * The desktop's `list_sessions` has no agent filter, so narrowing happens on the
 * client; what each `agent_type` string means is [SessionAgentTypes]' business.
 */
public enum class SessionAgentFilter {
    ALL,
    CODE,
    COWORK,
    ;

    public fun matches(agentType: String): Boolean = when (this) {
        ALL -> true
        CODE -> SessionAgentTypes.isCode(agentType)
        COWORK -> SessionAgentTypes.isCowork(agentType)
    }
}

public enum class SessionPermissionMode {
    ASK,
    AUTO,
    FULL_ACCESS,
}

/**
 * Why the permission section has no mode to show.
 *
 * A permission command that fails must not take the session down with it: the
 * transcript is still live and the user is still reading it, so the failure
 * stays inside the section that caused it. Which of the two it was decides
 * whether the shown mode is stale or simply unknown.
 */
public enum class PermissionModeFailure {
    /** `get_permission_mode` failed, so no mode is known. */
    LOAD,

    /** `set_permission_mode` failed; the mode still shown is the desktop's old one. */
    SAVE,
}

public enum class RemoteSessionFailureReason {
    TRANSPORT,
    SESSION_NOT_FOUND,
    PROTOCOL_MISMATCH,

    /**
     * The desktop has no workspace open, so `list_sessions` and `create_session`
     * cannot be answered. Detected before sending rather than by matching the
     * desktop's rejection text, which is localized on the desktop side.
     */
    NO_WORKSPACE,

    /**
     * The command reached the desktop and the desktop refused it. The reason it
     * gave is in [RemoteSessionUiState.Failed.remoteMessage] — the split matches
     * [com.bitfun.mobile.core.feature.pairing.PairingFailureReason.DesktopRejected].
     */
    REMOTE_REJECTED,

    /** The command never reached the relay. */
    NETWORK,

    /** The relay or the desktop did not answer in time. */
    TIMEOUT,

    /** The relay is throttling this client. */
    RATE_LIMITED,
}

public data class ComposerImage public constructor(
    public val id: String,
    public val dataUrl: String,
    public val mimeType: String,
)

public sealed interface RemoteSessionUiState {
    public data object Idle : RemoteSessionUiState

    public data object Loading : RemoteSessionUiState

    public data class Ready public constructor(
        public val sessions: List<RemoteSession>,
        public val selectedSessionId: String?,
        public val timeline: ChatTimelineState?,
        public val busy: Boolean,
        public val permissionMode: SessionPermissionMode?,
        /** Null while the permission section is either loading or settled. */
        public val permissionModeFailure: PermissionModeFailure?,
        /** The search text the visible list was produced with; empty when unfiltered. */
        public val query: String,
        public val agentFilter: SessionAgentFilter,
        /** Whether another `list_sessions` page is worth asking for. */
        public val hasMore: Boolean,
    ) : RemoteSessionUiState

    /**
     * @param remoteMessage verbatim text from the desktop, present only for
     * [RemoteSessionFailureReason.REMOTE_REJECTED]. It was written by the peer,
     * so it is not and cannot be localized here; apps show it as supporting
     * detail under their own heading for the reason.
     */
    public data class Failed public constructor(
        public val reason: RemoteSessionFailureReason,
        public val remoteMessage: String?,
    ) : RemoteSessionUiState {
        /** Most reasons carry no detail; a secondary constructor because Swift ignores Kotlin defaults. */
        public constructor(reason: RemoteSessionFailureReason) : this(reason, null)
    }
}

public sealed interface RemoteSessionIntent {
    public data object Load : RemoteSessionIntent

    public data object Refresh : RemoteSessionIntent

    /** Fetch the next page of the session list, keeping what is already shown. */
    public data object LoadMore : RemoteSessionIntent

    public data class Search public constructor(
        public val query: String,
    ) : RemoteSessionIntent

    public data class SetAgentFilter public constructor(
        public val filter: SessionAgentFilter,
    ) : RemoteSessionIntent

    public data class Open public constructor(
        public val sessionId: String,
    ) : RemoteSessionIntent

    /**
     * Create a session and open it.
     *
     * [instruction] is sent as the first message once the session exists, which
     * is how the HarmonyOS "new session with a prompt" flow works; an empty
     * instruction just leaves the session idle.
     */
    public data class CreateSession public constructor(
        public val agentType: String,
        public val title: String,
        public val instruction: String,
        public val modelId: String?,
    ) : RemoteSessionIntent {
        public constructor(agentType: String) : this(agentType, "", "", null)
    }

    public data class DeleteSession public constructor(
        public val sessionId: String,
    ) : RemoteSessionIntent

    public data class RenameSession public constructor(
        public val sessionId: String,
        public val title: String,
    ) : RemoteSessionIntent

    /** Answer a tool that asked the user a question, rather than approving it. */
    public data class AnswerQuestion public constructor(
        public val sessionId: String,
        public val toolId: String,
        public val answer: String,
    ) : RemoteSessionIntent

    public data class SendMessage public constructor(
        public val sessionId: String,
        public val content: String,
        public val images: List<ComposerImage>?,
    ) : RemoteSessionIntent {
        public constructor(sessionId: String, content: String) : this(sessionId, content, null)
    }

    public data class CancelTurn public constructor(
        public val sessionId: String,
        public val turnId: String?,
    ) : RemoteSessionIntent

    public data class ApproveTool public constructor(
        public val sessionId: String,
        public val toolId: String,
    ) : RemoteSessionIntent

    public data class RejectTool public constructor(
        public val sessionId: String,
        public val toolId: String,
        public val reason: String,
    ) : RemoteSessionIntent

    public data class CancelTool public constructor(
        public val sessionId: String,
        public val toolId: String,
        public val reason: String,
    ) : RemoteSessionIntent

    /**
     * No session id: `set_permission_mode` is addressed to the desktop, not to
     * one of its sessions, and the mode it sets applies to every session on it.
     */
    public data class SetPermissionMode public constructor(
        public val mode: SessionPermissionMode,
    ) : RemoteSessionIntent

    /**
     * Ask the desktop for its permission mode again.
     *
     * Separate from [Load] because it must not touch the session list: it is
     * both what the settings page asks on open and the retry it offers once a
     * load or a save has failed.
     */
    public data object RefreshPermissionMode : RemoteSessionIntent

    public data class SelectModel public constructor(
        public val sessionId: String,
        public val modelId: String,
    ) : RemoteSessionIntent

    public data object Stop : RemoteSessionIntent
}
