package com.bitfun.mobile.core.feature.session

/**
 * The rules the new-session screen runs on, ported from
 * `pages/state/RemoteCreateSessionState.ets`.
 *
 * The screen itself is two controls — a workspace and an instruction — but which
 * agent that adds up to is not obvious, and getting it wrong produces a session
 * that silently ignores the workspace the user picked. That decision lives here
 * so both clients make it the same way.
 */
public object CreateSessionPresenter {
    /** What the desktop calls its chat agent. Wire value, not UI copy. */
    public const val CHAT_AGENT: String = "Claw"

    /** The agent that is bound to a workspace on the desktop side. */
    public const val WORKSPACE_AGENT: String = "code"

    /**
     * Which agent a picked workspace implies.
     *
     * The desktop binds every chat session to its own assistant workspace and
     * ignores the requested `workspace_path`, so a picked workspace only holds
     * when it is paired with the code agent. No workspace means chat, which is
     * what the chat agent is for.
     */
    public fun agentType(workspacePath: String): String =
        if (workspacePath.trim().isEmpty()) CHAT_AGENT else WORKSPACE_AGENT

    /**
     * Whether the create button can fire, mirroring the source's `canSend()`.
     *
     * An instruction is required because creating alone leaves an empty session
     * on the desktop that the user then has to find and delete; a device is
     * required because the request has nowhere to go without one.
     */
    public fun canSubmit(instruction: String, deviceId: String, submitting: Boolean): Boolean =
        instruction.trim().isNotEmpty() && deviceId.trim().isNotEmpty() && !submitting
}
