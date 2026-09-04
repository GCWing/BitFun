package com.openbitfun.mobile.core.domain

/** Message-only interpretations of the relay's raw status value. */
internal object MessageStatusSemantics {
    fun isStreaming(status: String): Boolean = status.normalized() == "active"

    fun isFinalizing(status: String): Boolean = status.normalized() == "completed"

    fun isRetryableFailure(status: String): Boolean = status.normalized() == "failed"

    fun shouldHoldCompletedTurn(status: String): Boolean =
        status.normalized() in setOf("completed", "done", "success")
}

/** Tool-only ordering used to reject stale snapshots. */
internal object ToolStatusSemantics {
    fun shouldKeepPrevious(previous: String?, incoming: String?): Boolean =
        rank(incoming.orEmpty()) < rank(previous.orEmpty())

    private fun rank(status: String): Int =
        when (status.normalized()) {
            "pending_confirmation", "needs_confirmation", "pending" -> 1
            "running", "active" -> 2
            "completed", "success", "finished" -> 3
            "rejected", "cancelled", "canceled", "error", "failed" -> 4
            else -> 0
        }
}

private fun String.normalized(): String = lowercase()
