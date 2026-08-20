package com.bitfun.mobile.core.feature.session

import com.bitfun.mobile.core.domain.ChatMessage
import com.bitfun.mobile.core.protocol.ChatMessageItemResponse
import com.bitfun.mobile.core.protocol.RemoteToolStatusResponse
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MessageBlockPresentationTest {
    @Test
    fun aPlainAnswerHasNoBlocksSoTheAppKeepsItsFlatPath() {
        val blocks = messageBlocks(message(text = "Done."), false)

        assertTrue(blocks.isEmpty())
    }

    @Test
    fun theTurnKeepsTheOrderTheAgentProducedIt() {
        val blocks = messageBlocks(
            message(
                items = listOf(
                    item(type = "text", content = "Reading the config."),
                    item(tool = tool(id = "t-1", status = "completed")),
                    item(type = "text", content = "It sets the port to 8080."),
                ),
            ),
            false,
        )

        assertEquals(
            listOf("Reading the config.", "tools", "It sets the port to 8080."),
            blocks.map(::describe),
        )
    }

    @Test
    fun toolsRunBackToBackShareOneBlockSoTheyCanFold() {
        val blocks = messageBlocks(
            message(
                items = listOf(
                    item(tool = tool(id = "t-1", status = "completed")),
                    item(tool = tool(id = "t-2", status = "completed")),
                    item(type = "text", content = "Both read."),
                ),
            ),
            false,
        )

        val tools = blocks.filterIsInstance<MessageBlock.Tools>().single()
        assertEquals(listOf("t-1", "t-2"), tools.tools.map { it.id })
    }

    @Test
    fun aSubagentCarriesItsOwnTurnUnderIt() {
        val blocks = messageBlocks(
            message(
                items = listOf(
                    item(
                        type = "subagent",
                        content = "Audit the auth flow",
                        tool = tool(id = "t-1", name = "Task", status = "running"),
                        subItems = listOf(
                            item(tool = tool(id = "t-2", status = "completed")),
                            item(type = "text", content = "No leaks found."),
                        ),
                    ),
                ),
            ),
            false,
        )

        val subagent = blocks.single() as MessageBlock.Subagent
        assertEquals("Audit the auth flow", subagent.title)
        assertTrue(subagent.running)
        assertEquals(listOf("tools", "No leaks found."), subagent.children.map(::describe))
    }

    @Test
    fun aTaskToolIsTheSubagentRatherThanSomethingItDid() {
        // No `is_subagent` flag and no `subagent` type — only the tool's name.
        val blocks = messageBlocks(
            message(
                items = listOf(
                    item(
                        type = "tool",
                        tool = tool(
                            id = "t-1",
                            name = "Task",
                            status = "completed",
                            inputPreview = """{"description":"Sweep the migrations"}""",
                        ),
                    ),
                ),
            ),
            false,
        )

        val subagent = blocks.single() as MessageBlock.Subagent
        assertEquals("Sweep the migrations", subagent.title)
    }

    @Test
    fun liveReasoningMovesBelowTheOutputItCameBefore() {
        val items = listOf(
            item(type = "thinking", content = "which file first"),
            item(type = "text", content = "Starting with the manifest."),
            item(type = "thinking", content = "now the gradle file"),
        )

        val settled = messageBlocks(message(items = items), false)
        assertEquals(
            listOf("which file first", "Starting with the manifest.", "now the gradle file"),
            settled.map(::describe),
        )

        // Streaming: the reasoning being written is the only part worth
        // following, so it goes last and the round before it goes away.
        val live = messageBlocks(message(items = items), true)
        assertEquals(listOf("Starting with the manifest.", "now the gradle file"), live.map(::describe))
        assertTrue((live.last() as MessageBlock.Thinking).streaming)
    }

    @Test
    fun aToolReportedBothFlatAndInlineIsNotDrawnTwice() {
        val blocks = messageBlocks(
            message(
                tools = listOf(tool(id = "t-1", status = "completed")),
                items = listOf(item(tool = tool(id = "t-1", status = "completed"))),
            ),
            false,
        )

        assertEquals(listOf("t-1"), blocks.filterIsInstance<MessageBlock.Tools>().flatMap { it.tools }.map { it.id })
    }

    @Test
    fun aToolTheItemsNeverMentionIsStillDrawn() {
        val blocks = messageBlocks(
            message(
                tools = listOf(tool(id = "t-2", status = "running")),
                items = listOf(item(tool = tool(id = "t-1", status = "completed"))),
            ),
            false,
        )

        val ids = blocks.filterIsInstance<MessageBlock.Tools>().flatMap { it.tools }.map { it.id }
        assertEquals(listOf("t-1", "t-2"), ids)
    }

    @Test
    fun anIdlessToolIsMatchedByWhatItIsInstead() {
        // Some agents send no ids at all; matching on the id alone would then
        // treat every flat tool as uncovered and draw the whole run twice.
        val anonymous = tool(id = null, status = "completed", inputPreview = "ls -la")
        val blocks = messageBlocks(
            message(tools = listOf(anonymous), items = listOf(item(tool = anonymous))),
            false,
        )

        assertEquals(1, blocks.filterIsInstance<MessageBlock.Tools>().flatMap { it.tools }.size)
    }

    @Test
    fun theWaitingIndicatorIsOnlyForATurnThatHasProducedNothing() {
        assertTrue(isTyping(message(), true))
        assertTrue(!isTyping(message(), false))
        assertTrue(!isTyping(message(text = "On it."), true))
        assertTrue(!isTyping(message(thinking = "hmm"), true))
        assertTrue(!isTyping(message(tools = listOf(tool(id = "t-1", status = "running"))), true))
        assertTrue(!isTyping(message(items = listOf(item(type = "text", content = "hi"))), true))
    }
}

private fun describe(block: MessageBlock): String = when (block) {
    is MessageBlock.Text -> block.text
    is MessageBlock.Thinking -> block.text
    is MessageBlock.Tools -> "tools"
    is MessageBlock.Subagent -> "subagent"
}

private fun message(
    text: String = "",
    thinking: String? = null,
    tools: List<RemoteToolStatusResponse>? = null,
    items: List<ChatMessageItemResponse>? = null,
) = ChatMessage(
    id = "m-1",
    role = "assistant",
    text = text,
    status = "completed",
    renderVersion = null,
    turnId = null,
    detail = null,
    timestamp = null,
    thinking = thinking,
    tools = tools,
    items = items,
    images = null,
)

private fun item(
    type: String? = null,
    content: String? = null,
    tool: RemoteToolStatusResponse? = null,
    subItems: List<ChatMessageItemResponse>? = null,
) = ChatMessageItemResponse(type = type, content = content, tool = tool, subItems = subItems)

private fun tool(
    id: String?,
    status: String,
    name: String = "Read",
    inputPreview: String? = null,
) = RemoteToolStatusResponse(id = id, name = name, status = status, inputPreview = inputPreview)
