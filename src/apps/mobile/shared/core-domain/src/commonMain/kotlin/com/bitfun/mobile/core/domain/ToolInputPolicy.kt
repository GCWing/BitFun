package com.bitfun.mobile.core.domain

import com.bitfun.mobile.core.protocol.RelayJson
import com.bitfun.mobile.core.protocol.RemoteToolStatusResponse
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/** A file a tool named in its input, and what to call it on screen. */
public data class ToolFileTarget public constructor(
    public val path: String,
    public val label: String,
)

/**
 * What a tool was asked to act on, read out of its input.
 *
 * Ported from `summaryLabel` / `toolInputPayload` in
 * `pages/components/ToolStatusList.ets` and from `ToolFileReferenceResolver.ets`.
 * A tool row has one line to say what happened, and `{"file_path":"/a/b/c.kt",
 * "old_string":…}` is not that line — the parts worth reading are a file's name,
 * a command, or a pattern, so they are lifted out here rather than at each app.
 */
public object ToolInputPolicy {
    /** As much of the target as fits on one row beside the operation's name. */
    public const val SUMMARY_LIMIT: Int = 46

    /**
     * The target, compacted to one line, or empty when the input says nothing
     * beyond what the tool's own name already says.
     *
     * Empty rather than the tool name — which is what the source falls back to —
     * so the app can drop the separator instead of printing `Read file · Read`.
     */
    public fun summary(tool: RemoteToolStatusResponse): String {
        val payload = payload(tool)
        val fromPayload = summaryFromPayload(tool, payload)
        if (fromPayload.isNotEmpty()) return compact(fromPayload, SUMMARY_LIMIT)
        val preview = ToolStatusPolicy.inputText(tool)
        // JSON that got this far has no field worth quoting, and printing the
        // braces would spend the row on punctuation.
        if (preview.isNotEmpty() && !looksLikeJson(preview)) return compact(preview, SUMMARY_LIMIT)
        return ""
    }

    /**
     * The file this tool is working on, when opening it would show the user what
     * the tool did.
     *
     * Only the tools whose input names a single file qualify: a `Bash` that
     * happens to carry a `path` is not opening it, and a directory is not a
     * preview.
     */
    public fun fileTarget(tool: RemoteToolStatusResponse): ToolFileTarget? {
        if (!ToolNamePolicy.isFileRead(tool) && !ToolNamePolicy.isFileMutation(tool)) return null
        // A directory is not a preview, and a file that has just been deleted
        // cannot be opened — offering either is a link that fails when tapped.
        if (ToolNamePolicy.isDirectoryList(tool) || ToolNamePolicy.isDelete(tool)) return null
        val path = payload(tool)?.path().orEmpty().trim()
        if (path.isEmpty() || path.endsWith('/') || path.endsWith('\\')) return null
        return ToolFileTarget(path, basename(path))
    }

    private fun summaryFromPayload(tool: RemoteToolStatusResponse, payload: JsonObject?): String {
        if (payload == null) return ""
        if (ToolNamePolicy.isTodo(tool)) {
            val first = (payload["todos"] as? JsonArray)?.firstOrNull() as? JsonObject
            first?.text("content")?.let { return it }
        }
        if (ToolNamePolicy.isTask(tool)) {
            TASK_KEYS.forEach { key -> payload.text(key)?.let { return it } }
        }
        val path = payload.path()
        if (path.isNotEmpty()) return basename(path)
        TARGET_KEYS.forEach { key -> payload.text(key)?.let { return it } }
        return ""
    }

    private fun payload(tool: RemoteToolStatusResponse): JsonObject? {
        (tool.toolInput as? JsonObject)?.let { return it }
        val preview = tool.inputPreview.orEmpty().trim()
        if (!looksLikeJson(preview)) return null
        return runCatching { RelayJson.parseToJsonElement(preview) }.getOrNull() as? JsonObject
    }

    private fun JsonObject.path(): String =
        text("file_path") ?: text("filePath") ?: text("path") ?: ""

    private fun JsonObject.text(key: String): String? =
        (this[key] as? JsonPrimitive)?.takeIf { it.isString }?.content
            ?.trim()
            ?.takeIf(String::isNotEmpty)

    private fun looksLikeJson(value: String): Boolean {
        val text = value.trim()
        return text.startsWith('{') || text.startsWith('[')
    }

    /** The file's own name, with the desktop's scheme and any line marker off. */
    private fun basename(path: String): String {
        var normalized = path
            .removePrefix("computer://")
            .removePrefix("file://")
            .replace('\\', '/')
        val hash = normalized.lastIndexOf('#')
        if (hash > 0) normalized = normalized.substring(0, hash)
        normalized = normalized.replace(LINE_SUFFIX, "")
        return normalized.substringAfterLast('/').ifEmpty { normalized }
    }

    private fun compact(value: String, limit: Int): String {
        val text = value.replace('\n', ' ').replace(WHITESPACE, " ").trim()
        return if (text.length <= limit) text else text.take(limit) + "..."
    }

    private val WHITESPACE = Regex("\\s+")
    private val LINE_SUFFIX = Regex(":\\d+(?:-\\d+)?$")
    private val TASK_KEYS = listOf("description", "task", "prompt", "content")
    private val TARGET_KEYS = listOf(
        "description",
        "command",
        "cmd",
        "pattern",
        "regex",
        "query",
        "search_term",
        "needle",
        "title",
        "question",
        "prompt",
        "content",
        "url",
    )
}
