package com.bitfun.mobile.core.feature.session

import com.bitfun.mobile.core.domain.ChatMessage
import com.bitfun.mobile.core.domain.ChatSessionCursor
import com.bitfun.mobile.core.domain.ChatSyncPhase
import com.bitfun.mobile.core.domain.ChatTimelineState
import com.bitfun.mobile.core.protocol.ChatMessageItemResponse
import com.bitfun.mobile.core.protocol.RemoteModelCatalog
import com.bitfun.mobile.core.protocol.RemoteToolStatusResponse
import com.bitfun.mobile.core.protocol.RelayJson
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ConversationPresentationTest {
    @Test
    fun anEmptySessionGetsTheOneRowNobodyOwns() {
        val rows = timeline().conversationRows()

        assertEquals(listOf(ConversationRowKind.EMPTY), rows.map { it.kind })
    }

    @Test
    fun anActiveTurnStaysBelowTheMessageThatStartedIt() {
        val persisted = listOf(message("user-1", "user", "First"))
        val optimistic = listOf(message("local-1", "user", "Second"))
        val activeTurn = message("turn-1", "assistant", "Working")

        assertEquals(
            listOf("message-user-1", "pending-local-1", "active-turn-1"),
            timeline(persisted, optimistic, activeTurn, activeTurnAnchorId = "local-1")
                .conversationRows().map { it.id },
        )
        // With no anchor, the live turn follows the persisted messages instead.
        assertEquals(
            listOf("message-user-1", "active-turn-1", "pending-local-1"),
            timeline(persisted, optimistic, activeTurn).conversationRows().map { it.id },
        )
    }

    @Test
    fun aMessageStillInFlightIsShownAsPendingUntilItsTwinArrives() {
        val optimistic = timeline(optimistic = listOf(message("local-1", "user", "ship it")))
        assertEquals(listOf(true), optimistic.conversationRows().map { it.pending })

        // The same message identity persisted: one row, no longer pending. Reading
        // persistedMessages directly would have shown it twice.
        val persisted = timeline(
            persisted = listOf(message("local-1", "user", "ship it")),
            optimistic = listOf(message("local-1", "user", "ship it")),
        )
        val rows = persisted.conversationRows()
        assertEquals(1, rows.size)
        assertEquals(false, rows.single().pending)
    }

    @Test
    fun reasoningIsNotPrintedTwiceAsTheAnswer() {
        // Some agents copy the reasoning into `text` until the answer starts.
        val rows = timeline(
            persisted = listOf(
                message("m-1", "assistant", "weighing options", thinking = "weighing options"),
            ),
        ).conversationRows()

        assertEquals("", rows.single().text)
        assertEquals("weighing options", rows.single().thinking)
    }

    @Test
    fun anAnswerLeftInTheItemListStillReachesTheBubble() {
        val rows = timeline(
            persisted = listOf(
                message(
                    "m-1",
                    "assistant",
                    "",
                    items = listOf(
                        ChatMessageItemResponse(type = "thinking", content = "hmm"),
                        ChatMessageItemResponse(type = "text", content = "Done."),
                    ),
                ),
            ),
        ).conversationRows()

        assertEquals("Done.", rows.single().text)
    }

    @Test
    fun aToolWaitingOnApprovalOffersApproveAndReject() {
        val card = cardsFor(tool(status = "pending_confirmation", name = "Bash")).single()

        assertEquals(ToolPhase.PENDING_CONFIRMATION, card.phase)
        assertEquals(setOf(ToolAction.APPROVE, ToolAction.REJECT), card.actions)
    }

    @Test
    fun aRunningToolOffersCancelAndNothingElse() {
        val card = cardsFor(tool(status = "running")).single()

        assertEquals(ToolPhase.RUNNING, card.phase)
        assertEquals(setOf(ToolAction.CANCEL), card.actions)
    }

    @Test
    fun aQuestionIsAnsweredRatherThanApproved() {
        val card = cardsFor(
            tool(
                name = "AskUserQuestion",
                status = "sent",
                inputPreview = """{"question":"Which branch?"}""",
            ),
        ).single()

        assertEquals(setOf(ToolAction.ANSWER), card.actions)
        assertEquals("Which branch?", card.question)
    }

    @Test
    fun aStructuredQuestionKeepsTheLegacyPromptAndExposesChoices() {
        val card = cardsFor(
            tool(
                name = "AskUserQuestion",
                status = "sent",
                inputPreview = "Which branch?",
                toolInput = RelayJson.parseToJsonElement(
                    """{"questions":[{"header":"Dropped","options":[{"label":"ignored"}]},{"header":"Branch","question":"Which branch?","options":[{"label":"main","description":"Stable"},{"label":"dev"}],"multiSelect":true}]}""",
                ),
            ),
        ).single()

        assertEquals("Which branch?", card.question)
        assertEquals(
            listOf(
                ToolQuestion(
                    index = 1,
                    header = "Branch",
                    question = "Which branch?",
                    options = listOf(QuestionOption("main", "Stable"), QuestionOption("dev", null)),
                    multiSelect = true,
                ),
            ),
            card.questions,
        )
    }

    @Test
    fun aFinishedToolOffersNothing() {
        val card = cardsFor(tool(status = "completed", resultPreview = "ok")).single()

        assertEquals(ToolPhase.COMPLETED, card.phase)
        assertTrue(card.actions.isEmpty())
        assertEquals("ok", card.output)
    }

    @Test
    fun successMapsToCompletedWithItsOutput() {
        val card = cardsFor(tool(status = "success", resultPreview = "ok")).single()

        assertEquals(ToolPhase.COMPLETED, card.phase)
        assertTrue(card.actions.isEmpty())
        assertEquals("ok", card.output)
    }

    @Test
    fun toolExpandabilityComesFromTheStatusContract() {
        assertEquals(true, toolCard(tool(status = "completed")).expandable)
        assertEquals(true, toolCard(tool(status = "running")).expandable)
        assertEquals(false, toolCard(tool(status = "queued", name = "Bash")).expandable)
        assertEquals(
            true,
            toolCard(tool(name = "AskUserQuestion", status = "sent")).expandable,
        )
    }

    @Test
    fun aToolWithNoIdIsShownButNotActionable() {
        // Every action is addressed by id, so buttons here could not be delivered.
        val card = cardsFor(tool(id = null, status = "pending_confirmation")).single()

        assertEquals(ToolPhase.PENDING_CONFIRMATION, card.phase)
        assertTrue(card.actions.isEmpty())
    }

    @Test
    fun aToolReportedBothFlatAndInlineIsOneCard() {
        val rows = timeline(
            persisted = listOf(
                message(
                    "m-1",
                    "assistant",
                    "",
                    tools = listOf(tool(status = "running")),
                    items = listOf(
                        ChatMessageItemResponse(
                            type = "tool",
                            tool = tool(status = "completed", resultPreview = "ok"),
                        ),
                    ),
                ),
            ),
        ).conversationRows()

        val card = rows.single().tools.single()
        // The inline copy is the one the agent keeps updating, so it wins.
        assertEquals(ToolPhase.COMPLETED, card.phase)
    }

    private fun cardsFor(tool: RemoteToolStatusResponse): List<ToolCard> =
        timeline(persisted = listOf(message("m-1", "assistant", "working", tools = listOf(tool))))
            .conversationRows()
            .single()
            .tools
}

private fun timeline(
    persisted: List<ChatMessage> = emptyList(),
    optimistic: List<ChatMessage> = emptyList(),
    activeTurn: ChatMessage? = null,
    activeTurnAnchorId: String = "",
) = ChatTimelineState(
    sessionId = "s-1",
    persistedMessages = persisted,
    optimisticMessages = optimistic,
    activeTurn = activeTurn,
    activeTurnAnchorId = activeTurnAnchorId,
    syncPhase = ChatSyncPhase.IDLE,
    cursor = ChatSessionCursor(0, 0, 0),
    modelCatalog = RemoteModelCatalog(version = 0),
    selectedModelId = "",
)

private fun message(
    id: String,
    role: String,
    text: String,
    thinking: String? = null,
    tools: List<RemoteToolStatusResponse>? = null,
    items: List<ChatMessageItemResponse>? = null,
) = ChatMessage(
    id = id,
    role = role,
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

private fun tool(
    id: String? = "tool-1",
    name: String = "Bash",
    status: String,
    inputPreview: String? = null,
    resultPreview: String? = null,
    toolInput: kotlinx.serialization.json.JsonElement? = null,
) = RemoteToolStatusResponse(
    id = id,
    name = name,
    status = status,
    inputPreview = inputPreview,
    resultPreview = resultPreview,
    toolInput = toolInput,
)
