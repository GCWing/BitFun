package com.openbitfun.mobile.core.feature.session

/** Notification identity includes the host; matching session IDs on two hosts are unrelated. */
public data class TaskCompletionIdentity(public val target: String, public val sessionId: String, public val turnId: String)

/** Only explicitly successful, previously observed turns may notify. Missing data is not completion. */
public class TaskCompletionPolicy {
    private var tracked: TaskCompletionIdentity? = null
    private var background: Boolean = false
    private var lastFinished: TaskCompletionIdentity? = null

    public fun setBackground(value: Boolean) { background = value }
    public fun reset() { tracked = null; lastFinished = null }
    public fun hasPending(): Boolean = tracked != null

    public fun observe(state: RemoteSessionUiState, target: String): TaskCompletionIdentity? {
        val ready = state as? RemoteSessionUiState.Ready ?: return null
        val timeline = ready.timeline ?: return null
        val active = timeline.activeTurn
        val activeId = active?.turnId?.takeIf { it.isNotBlank() }
        if (activeId != null) {
            val identity = TaskCompletionIdentity(target, timeline.sessionId, activeId)
            val status = active.status.lowercase()
            if (status in listOf("active", "running", "streaming", "pending")) track(identity)
            val result = finish(identity, status)
            if (result != null) return result
        }
        val watching = tracked ?: return null
        if (watching.target != target || watching.sessionId != timeline.sessionId) return null
        val terminal = timeline.persistedMessages.lastOrNull {
            it.role == "assistant" && it.turnId == watching.turnId
        } ?: return null
        return finish(watching, terminal.status.lowercase())
    }

    public fun track(identity: TaskCompletionIdentity) {
        if (identity.target.isBlank() || identity.sessionId.isBlank() || identity.turnId.isBlank()) return
        if (identity == lastFinished) return
        if (background && tracked != null && tracked != identity) return
        tracked = identity
    }

    public fun finish(identity: TaskCompletionIdentity, status: String): TaskCompletionIdentity? {
        if (tracked != identity) return null
        if (status !in listOf("completed", "done", "success", "failed", "error", "cancelled", "canceled")) return null
        tracked = null
        lastFinished = identity
        return identity.takeIf { background && status in listOf("completed", "done", "success") }
    }
}
