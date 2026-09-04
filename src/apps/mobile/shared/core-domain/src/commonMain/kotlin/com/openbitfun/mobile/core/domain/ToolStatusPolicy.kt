package com.openbitfun.mobile.core.domain

import com.openbitfun.mobile.core.protocol.RelayJson
import com.openbitfun.mobile.core.protocol.RemoteToolStatusResponse
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * What the relay's raw tool `status` means, and what a client may do about it.
 *
 * Ported from the predicates at the bottom of `pages/components/ToolStatusList.ets`.
 * The relay has never narrowed this field to an enum — different agents spell the
 * same state `completed` / `done` / `sent` / `success` (with legacy `finished`), and confirmation arrives as either
 * `pending_confirmation` or `needs_confirmation` — so the vocabulary is matched
 * here once instead of at every call site.
 */
public object ToolStatusPolicy {
    /** The tool is waiting for the user to approve or reject it. */
    public fun isPendingConfirmation(tool: RemoteToolStatusResponse): Boolean =
        tool.status.normalized() in PENDING

    public fun isRunning(tool: RemoteToolStatusResponse): Boolean =
        tool.status.normalized() in RUNNING

    /** Whether the tool has completed, including `success` and legacy `finished`. */
    public fun isCompleted(tool: RemoteToolStatusResponse): Boolean =
        tool.status.normalized() in COMPLETED

    /** Whether the tool has a status-driven expandable card, regardless of preview content. */
    public fun isExpandable(tool: RemoteToolStatusResponse): Boolean =
        isFailed(tool) || isPendingConfirmation(tool) || isQuestion(tool) || isRunning(tool) ||
            isCompleted(tool) || isCancelled(tool)

    /**
     * Whether the tool has content to reveal. This is separate from [isExpandable]:
     * expandability is a status-driven typed domain fact, not a preview-driven one,
     * for a later Android UI consumer (P3).
     */
    public fun hasPreview(tool: RemoteToolStatusResponse): Boolean =
        inputText(tool).isNotEmpty() || outputText(tool).isNotEmpty()

    public fun isCancelled(tool: RemoteToolStatusResponse): Boolean =
        tool.status.normalized() in CANCELLED

    /**
     * Whether the tool failed.
     *
     * Output is inspected as well as status because some agents finish a tool
     * with `completed` and put the failure in `stderr` or `error_preview`.
     */
    public fun isFailed(tool: RemoteToolStatusResponse): Boolean =
        tool.status.normalized() in FAILED ||
            !tool.errorPreview.isNullOrEmpty() ||
            !tool.stderr.isNullOrEmpty()

    /**
     * Whether the tool is asking the user a question rather than for permission.
     *
     * `AskUserQuestion` is answered with `answer_question`, not `approve_tool`,
     * so the two are different affordances even though both block the turn.
     *
     * `sent` counts as still asking, which is why the terminal set here is not
     * [isCompleted]'s: on a question `sent` means the prompt reached the user and
     * the turn is waiting on them. The HarmonyOS predicate draws the same line.
     */
    public fun isQuestion(tool: RemoteToolStatusResponse): Boolean =
        tool.name == QUESTION_TOOL && tool.status.normalized() !in QUESTION_TERMINAL

    /**
     * The question to put to the user.
     *
     * `input_preview` is a string, and whether it holds JSON is up to the agent;
     * when it does not parse, the preview itself is the best question available.
     * Returns null only when there is nothing at all to show, so callers can fall
     * back to their own wording.
     */
    public fun questionPrompt(tool: RemoteToolStatusResponse): String? {
        val preview = inputText(tool)
        if (preview.isEmpty()) return null
        return parsedQuestion(preview) ?: preview
    }

    /** What the tool was asked to do; falls back to the raw input when there is no preview. */
    public fun inputText(tool: RemoteToolStatusResponse): String {
        tool.inputPreview?.takeIf(String::isNotEmpty)?.let { return it.trim() }
        return tool.toolInput?.toString().orEmpty().trim()
    }

    /**
     * The tool's output, assembled and capped the way the HarmonyOS card does it.
     *
     * The cap is not cosmetic: a tool that dumped a whole file would otherwise
     * push the rest of the turn off the screen.
     */
    public fun outputText(tool: RemoteToolStatusResponse): String {
        val parts = listOfNotNull(
            tool.resultPreview,
            tool.errorPreview,
            tool.stdout,
            tool.stderr,
            tool.toolOutput?.toString(),
        ).filter(String::isNotEmpty)
        val text = parts.joinToString("\n").trim()
        return if (text.length <= OUTPUT_CAP) text else text.take(OUTPUT_CAP) + "..."
    }

    private fun parsedQuestion(preview: String): String? {
        val root = runCatching { RelayJson.parseToJsonElement(preview) }.getOrNull() ?: return null
        val payload = root as? JsonObject ?: return null
        payload.text("question")?.let { return it }
        payload.text("prompt")?.let { return it }
        val first = (payload["questions"] as? JsonArray)?.firstOrNull()?.jsonObject ?: return null
        return first.text("question") ?: first.text("header")
    }

    private fun JsonObject.text(key: String): String? =
        (this[key] as? JsonPrimitive)?.takeIf { it.isString }?.jsonPrimitive?.content
            ?.trim()
            ?.takeIf(String::isNotEmpty)

    private fun String?.normalized(): String = orEmpty().lowercase()

    private const val QUESTION_TOOL = "AskUserQuestion"
    private const val OUTPUT_CAP = 480
    private val PENDING = setOf("pending_confirmation", "needs_confirmation")
    private val RUNNING = setOf("running", "active")
    private val COMPLETED = setOf("completed", "done", "sent", "success", "finished")
    private val CANCELLED = setOf("cancelled", "canceled", "rejected")
    private val FAILED = setOf("failed", "error")
    private val QUESTION_TERMINAL = setOf("completed", "done") + CANCELLED + FAILED
}
