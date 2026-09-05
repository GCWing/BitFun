package com.openbitfun.mobile.core.feature.session

import com.openbitfun.mobile.core.domain.ToolNamePolicy
import com.openbitfun.mobile.core.domain.ToolStatusPolicy
import com.openbitfun.mobile.core.protocol.RemoteToolStatusResponse

/**
 * The picture a tool row leads with, ported from `toolGlyphKind` in
 * `pages/components/ToolStatusList.ets`.
 *
 * A turn runs a dozen tools and the user reads the column rather than each row,
 * so what a tool did has to be legible before its label is. The order the source
 * tests these in is load-bearing — a `Delete` is also a mutation, and the more
 * specific answer is the useful one.
 */
public enum class ToolKind {
    QUESTION,
    TODO,
    TASK,
    GIT,
    DELETE,
    DIFF,
    PATCH,
    CREATE,
    MUTATE,
    FOLDER,
    DOCUMENT,
    SEARCH,
    WEB,
    COMMAND,

    /** Nothing recognised; the row falls back to the tool's own name. */
    GENERIC,
}

/**
 * What the row says the tool is doing, as a value rather than a sentence.
 *
 * The wording lives in each app's resources (design doc §4.3) — this only fixes
 * which of them applies.
 */
public enum class ToolOperation {
    UPDATE_TODOS,
    START_TASK,
    READ_FILE,
    WRITE_FILE,
    DELETE_FILE,
    VIEW_DIFF,
    EDIT_FILE,
    RUN_COMMAND,
    SEARCH_WEB,
    OPEN_WEB,
    SEARCH_CODE,
    ASK_CONFIRMATION,

    /** No better name than the tool's own; apps print [ToolCard.name]. */
    UNKNOWN,
}

/**
 * One entry in the tool column: either a tool, or the several the user does not
 * need to read one by one.
 */
public sealed interface ToolRow {
    public val id: String

    public data class Single public constructor(
        public val tool: ToolCard,
    ) : ToolRow {
        override val id: String get() = tool.id.ifEmpty { tool.name }
    }

    /**
     * Consecutive finished lookups, folded into one line.
     *
     * An agent reading six files before answering produces six rows that say
     * nothing individually; the source collapses them and so does this. Only
     * finished ones — anything still running, failed, or waiting on the user is
     * why the user is looking at the column at all.
     */
    public data class Collapsed public constructor(
        public val tools: List<ToolCard>,
        public val readCount: Int,
        public val searchCount: Int,
    ) : ToolRow {
        override val id: String get() = "collapsed-${tools.first().id}-${tools.size}"
    }
}

/** Fewer than this many in a row is not a wall worth folding. */
private const val COLLAPSE_THRESHOLD = 2

/**
 * The tool column as rows, ported from `toolRenderEntries` in
 * `pages/components/ToolStatusList.ets`.
 */
public fun collapseToolRows(tools: List<ToolCard>): List<ToolRow> {
    val rows = mutableListOf<ToolRow>()
    val pending = mutableListOf<ToolCard>()

    fun flush() {
        if (pending.isEmpty()) return
        if (pending.size < COLLAPSE_THRESHOLD) {
            pending.forEach { rows += ToolRow.Single(it) }
        } else {
            rows += ToolRow.Collapsed(
                tools = pending.toList(),
                readCount = pending.count { it.kind == ToolKind.DOCUMENT || it.kind == ToolKind.FOLDER },
                searchCount = pending.count { it.kind == ToolKind.SEARCH },
            )
        }
        pending.clear()
    }

    tools.forEach { tool ->
        if (tool.isCollapsibleLookup()) {
            pending += tool
        } else {
            flush()
            rows += ToolRow.Single(tool)
        }
    }
    flush()
    return rows
}

private fun ToolCard.isCollapsibleLookup(): Boolean {
    if (actions.isNotEmpty()) return false
    if (phase != ToolPhase.COMPLETED && phase != ToolPhase.CANCELLED) return false
    return kind == ToolKind.DOCUMENT || kind == ToolKind.FOLDER || kind == ToolKind.SEARCH
}

internal fun toolKind(tool: RemoteToolStatusResponse): ToolKind = when {
    ToolNamePolicy.isQuestionLike(tool) -> ToolKind.QUESTION
    ToolNamePolicy.isTodo(tool) -> ToolKind.TODO
    ToolNamePolicy.isTask(tool) -> ToolKind.TASK
    ToolNamePolicy.isGit(tool) -> ToolKind.GIT
    ToolNamePolicy.isDelete(tool) -> ToolKind.DELETE
    ToolNamePolicy.isDiff(tool) -> ToolKind.DIFF
    ToolNamePolicy.isPatch(tool) -> ToolKind.PATCH
    ToolNamePolicy.isFileCreate(tool) -> ToolKind.CREATE
    ToolNamePolicy.isFileMutation(tool) -> ToolKind.MUTATE
    ToolNamePolicy.isDirectoryList(tool) -> ToolKind.FOLDER
    ToolNamePolicy.isFileRead(tool) -> ToolKind.DOCUMENT
    ToolNamePolicy.isSearch(tool) -> ToolKind.SEARCH
    ToolNamePolicy.isWebFetch(tool) -> ToolKind.WEB
    ToolNamePolicy.isCommand(tool) -> ToolKind.COMMAND
    else -> ToolKind.GENERIC
}

internal fun toolOperation(tool: RemoteToolStatusResponse): ToolOperation = when {
    ToolNamePolicy.isTodo(tool) -> ToolOperation.UPDATE_TODOS
    ToolNamePolicy.isTask(tool) -> ToolOperation.START_TASK
    ToolNamePolicy.isFileRead(tool) -> ToolOperation.READ_FILE
    ToolNamePolicy.isFileCreate(tool) -> ToolOperation.WRITE_FILE
    ToolNamePolicy.isDelete(tool) -> ToolOperation.DELETE_FILE
    ToolNamePolicy.isDiff(tool) -> ToolOperation.VIEW_DIFF
    ToolNamePolicy.isFileMutation(tool) -> ToolOperation.EDIT_FILE
    ToolNamePolicy.isCommand(tool) -> ToolOperation.RUN_COMMAND
    ToolNamePolicy.isWebSearch(tool) -> ToolOperation.SEARCH_WEB
    ToolNamePolicy.isWebFetch(tool) -> ToolOperation.OPEN_WEB
    ToolNamePolicy.isSearch(tool) -> ToolOperation.SEARCH_CODE
    ToolStatusPolicy.isQuestion(tool) || ToolNamePolicy.isQuestionLike(tool) -> ToolOperation.ASK_CONFIRMATION
    else -> ToolOperation.UNKNOWN
}
