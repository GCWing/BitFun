package com.openbitfun.mobile.core.feature.session

import com.openbitfun.mobile.core.domain.ChatMessage
import com.openbitfun.mobile.core.domain.ToolInputPolicy
import com.openbitfun.mobile.core.domain.ToolNamePolicy
import com.openbitfun.mobile.core.domain.ToolStatusPolicy
import com.openbitfun.mobile.core.protocol.ChatMessageItemResponse
import com.openbitfun.mobile.core.protocol.RemoteToolStatusResponse

/**
 * One piece of an agent turn, in the order the agent produced it.
 *
 * Ported from `structuredGroups` in `pages/components/ChatMessageBubble.ets`. A
 * turn is not a paragraph followed by a list of tools — it is a sequence: read
 * two files, say something about them, run a command, say what it printed.
 * Flattening that into "all the text, then all the tools" reads as if the agent
 * explained itself before doing any of the work.
 */
public sealed interface MessageBlock {
    public val id: String

    /** Prose, rendered as markdown. */
    public data class Text public constructor(
        override val id: String,
        public val text: String,
        /** Tokens are still arriving for this block specifically. */
        public val streaming: Boolean,
    ) : MessageBlock

    /** The model's reasoning; apps collapse this by default. */
    public data class Thinking public constructor(
        override val id: String,
        public val text: String,
        public val streaming: Boolean,
    ) : MessageBlock

    /** Tools the agent ran back to back, kept together so they can fold. */
    public data class Tools public constructor(
        override val id: String,
        public val tools: List<ToolCard>,
    ) : MessageBlock

    /**
     * A subagent's own turn, nested inside this one.
     *
     * [title] is empty when the agent gave it none — apps name it themselves
     * rather than printing a relay string.
     */
    public data class Subagent public constructor(
        override val id: String,
        public val title: String,
        /** Its tool is still going, so the app shows it is being waited on. */
        public val running: Boolean,
        public val text: String,
        public val children: List<MessageBlock>,
    ) : MessageBlock
}

/**
 * A message as ordered blocks, or empty when it has no structure worth walking.
 *
 * Empty is not "nothing to draw": it means the message is a plain answer, and
 * the app draws [ConversationRow.text] and [ConversationRow.tools] as before.
 * That is the same fork `shouldRenderStructuredItems` makes.
 */
internal fun messageBlocks(message: ChatMessage, streaming: Boolean): List<MessageBlock> {
    val items = message.items.orEmpty()
    if (items.none(::isRenderable)) return emptyList()

    val blocks = walk(items, message.id, streaming)
    val uncovered = uncoveredTools(message)
    val withTail = if (uncovered.isEmpty()) {
        blocks
    } else {
        blocks + MessageBlock.Tools("${message.id}-tail-tools", uncovered.map(::toolCard))
    }
    return pinLiveThinking(withTail, message, streaming)
}

/**
 * Whether the turn has produced nothing at all yet.
 *
 * The three dots stand in for the first token; once anything has arrived they
 * would only be saying what the arriving text already says.
 */
internal fun isTyping(message: ChatMessage, streaming: Boolean): Boolean {
    if (!streaming) return false
    if (message.text.trim().isNotEmpty()) return false
    if (!message.thinking.isNullOrBlank()) return false
    if (!message.tools.isNullOrEmpty()) return false
    return message.items.orEmpty().none(::isRenderable)
}

private fun walk(items: List<ChatMessageItemResponse>, path: String, streaming: Boolean): List<MessageBlock> {
    val blocks = mutableListOf<MessageBlock>()
    val toolRun = mutableListOf<RemoteToolStatusResponse>()
    var toolStart = 0
    val lastIndex = items.indexOfLast(::isRenderable)

    fun flushTools() {
        if (toolRun.isEmpty()) return
        blocks += MessageBlock.Tools("$path-tools-$toolStart", toolRun.map(::toolCard))
        toolRun.clear()
    }

    items.forEachIndexed { index, entry ->
        val live = streaming && index == lastIndex
        // A subagent is checked before its tool: `Task` arrives as a tool entry
        // and is the subagent, rather than something the subagent did.
        if (isSubagent(entry)) {
            flushTools()
            blocks += MessageBlock.Subagent(
                id = "$path-$index-subagent",
                title = subagentTitle(entry),
                running = entry.tool?.let(ToolStatusPolicy::isRunning) == true,
                text = entry.content.orEmpty().trim(),
                children = walk(entry.subItems.orEmpty(), "$path-$index", streaming && live),
            )
            return@forEachIndexed
        }
        val tool = entry.tool
        if (tool != null) {
            if (toolRun.isEmpty()) toolStart = index
            toolRun += tool
            return@forEachIndexed
        }
        flushTools()
        when {
            isThinking(entry) -> blocks += MessageBlock.Thinking(
                id = "$path-$index-thinking",
                text = entry.content.orEmpty().trim(),
                streaming = live,
            )

            isText(entry) -> blocks += MessageBlock.Text(
                id = "$path-$index-text",
                text = entry.content.orEmpty().trim(),
                streaming = live,
            )
        }
        entry.subItems?.takeIf(List<ChatMessageItemResponse>::isNotEmpty)?.let { children ->
            blocks += walk(children, "$path-$index", streaming && live)
        }
    }
    flushTools()
    return blocks
}

/**
 * While the turn is live, the newest reasoning goes last and the older rounds
 * of it go away.
 *
 * Reasoning that is still being written is the only part of a live turn worth
 * following, and leaving it where it was produced pushes it off screen behind
 * the output it came before.
 */
private fun pinLiveThinking(
    blocks: List<MessageBlock>,
    message: ChatMessage,
    streaming: Boolean,
): List<MessageBlock> {
    if (!streaming) return blocks
    val live = blocks.filterIsInstance<MessageBlock.Thinking>().lastOrNull()
        ?: return blocks
    val rest = blocks.filter { it !is MessageBlock.Thinking }
    return rest + live.copy(id = "${message.id}-live-thinking", streaming = true)
}

/**
 * The tools the relay reported flat that no item already accounts for.
 *
 * The same turn arrives twice — once in `tools`, once inline in `items` — and
 * which is populated depends on the agent. Drawing both would show every tool
 * twice on the agents that send both.
 */
private fun uncoveredTools(message: ChatMessage): List<RemoteToolStatusResponse> {
    val flat = message.tools.orEmpty()
    if (flat.isEmpty()) return emptyList()
    val covered = mutableSetOf<String>()
    fun collect(items: List<ChatMessageItemResponse>) {
        items.forEach { entry ->
            entry.tool?.let { covered += fingerprint(it) }
            entry.subItems?.let(::collect)
        }
    }
    collect(message.items.orEmpty())
    return flat.filter { fingerprint(it) !in covered }
}

/** Id when the agent gave one, and what the tool is otherwise. */
private fun fingerprint(tool: RemoteToolStatusResponse): String {
    tool.id?.takeIf(String::isNotEmpty)?.let { return "id:$it" }
    return listOf(
        ToolNamePolicy.normalized(tool),
        tool.status.orEmpty(),
        tool.inputPreview.orEmpty(),
        tool.resultPreview.orEmpty(),
        tool.errorPreview.orEmpty(),
        tool.exitCode?.toString().orEmpty(),
    ).joinToString("|")
}

private fun isRenderable(entry: ChatMessageItemResponse): Boolean =
    isThinking(entry) || isText(entry) || isSubagent(entry) || entry.tool != null ||
        entry.subItems.orEmpty().any(::isRenderable)

private fun isThinking(entry: ChatMessageItemResponse): Boolean =
    entry.type.orEmpty().lowercase() == "thinking" && entry.content.orEmpty().isNotBlank()

private fun isText(entry: ChatMessageItemResponse): Boolean {
    if (entry.content.orEmpty().isBlank()) return false
    if (entry.tool != null || isThinking(entry) || isSubagent(entry)) return false
    return entry.type.orEmpty().lowercase() in TEXT_TYPES
}

private fun isSubagent(entry: ChatMessageItemResponse): Boolean {
    if (entry.isSubagent == true) return true
    if (entry.type.orEmpty().lowercase() in SUBAGENT_TYPES) return true
    return entry.tool?.let(ToolNamePolicy::isTask) == true
}

/**
 * What to call the subagent: what it said, if that is short enough to be a
 * heading rather than a paragraph, else the task it was handed.
 */
private fun subagentTitle(entry: ChatMessageItemResponse): String {
    val content = entry.content.orEmpty().trim()
    if (content.isNotEmpty() && content.length <= TITLE_LIMIT) return content
    val tool = entry.tool ?: return ""
    // `summary` on a `Task` reads its `description`, which is the subagent's
    // brief — the closest thing to a name it was given.
    return ToolInputPolicy.summary(tool).ifEmpty { tool.name.orEmpty() }
}

private const val TITLE_LIMIT = 80
private val TEXT_TYPES = setOf("text", "message", "")
private val SUBAGENT_TYPES = setOf("subagent", "agent")
