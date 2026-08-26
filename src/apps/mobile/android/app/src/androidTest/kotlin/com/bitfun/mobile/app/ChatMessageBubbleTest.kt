package com.bitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.platform.app.InstrumentationRegistry
import com.bitfun.mobile.app.ui.chat.ChatMessageBubble
import com.bitfun.mobile.app.ui.chat.message.SUBAGENT_GROUP_TEST_TAG
import com.bitfun.mobile.app.ui.chat.message.TYPING_DOTS_TEST_TAG
import com.bitfun.mobile.core.feature.session.ConversationRow
import com.bitfun.mobile.core.feature.session.ConversationRowKind
import com.bitfun.mobile.core.feature.session.MessageBlock
import com.bitfun.mobile.core.feature.session.ToolCard
import com.bitfun.mobile.core.feature.session.ToolKind
import com.bitfun.mobile.core.feature.session.ToolOperation
import com.bitfun.mobile.core.feature.session.ToolPhase
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

/**
 * A turn as the agent produced it, ported from `ChatMessageBubble.ets`.
 *
 * What is worth pinning is the ordering: a turn that worked in steps has to read
 * as steps. The flat bubble this replaced put every tool below every paragraph,
 * so the agent appeared to have explained the whole job before touching any of
 * it — which is the one thing about the transcript that is not true.
 */
class ChatMessageBubbleTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun aTurnThatWorkedInStepsIsDrawnInSteps() {
        composeRule.setContent {
            Bubble(
                row(
                    kind = ConversationRowKind.ASSISTANT,
                    blocks = listOf(
                        MessageBlock.Text("b1", "Checking the manifest.", false),
                        MessageBlock.Tools("b2", listOf(runningTool())),
                        MessageBlock.Text("b3", "It targets API 35.", false),
                    ),
                ),
            )
        }

        val first = composeRule.onNodeWithText("Checking the manifest.").getUnclippedBoundsInRoot()
        val tool = composeRule.onNodeWithText("Running \"AndroidManifest.xml\"").getUnclippedBoundsInRoot()
        val second = composeRule.onNodeWithText("It targets API 35.").getUnclippedBoundsInRoot()

        assertTrue(first.top < tool.top)
        assertTrue(tool.top < second.top)
    }

    @Test
    fun aSubagentsWorkIsBoxedApartFromTheAgentThatStartedIt() {
        composeRule.setContent {
            Bubble(
                row(
                    kind = ConversationRowKind.ASSISTANT,
                    blocks = listOf(
                        MessageBlock.Subagent(
                            id = "b1",
                            title = "Audit the auth flow",
                            running = false,
                            text = "",
                            children = listOf(MessageBlock.Text("b1-1", "No leaks found.", false)),
                        ),
                    ),
                ),
            )
        }

        composeRule.onNodeWithTag(SUBAGENT_GROUP_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithText("Audit the auth flow").assertIsDisplayed()
        composeRule.onNodeWithText("No leaks found.").assertIsDisplayed()
    }

    @Test
    fun aFailureSaysWhichHalfOfTheExchangeFailed() {
        composeRule.setContent {
            Bubble(row(kind = ConversationRowKind.ASSISTANT, text = "Half an ans", showRetry = true))
        }

        // The agent's reply started and stopped; it was never "not delivered".
        composeRule.onNodeWithText("Reply interrupted.").assertIsDisplayed()
        composeRule.onNodeWithText("Retry").assertIsDisplayed()
    }

    @Test
    fun aMessageThatNeverLeftTheDeviceUsesTheSendFailureCopy() {
        composeRule.setContent {
            Bubble(row(kind = ConversationRowKind.USER, text = "ship it", showRetry = true))
        }

        composeRule.onNodeWithText(string(R.string.chat_send_failed)).assertIsDisplayed()
    }

    @Test
    fun theWaitingDotsStandInForTheFirstTokenAndNothingMore() {
        // The dots never settle, so the clock is driven by hand rather than
        // waiting for an idle that will not come.
        composeRule.mainClock.autoAdvance = false

        composeRule.setContent {
            Bubble(row(kind = ConversationRowKind.ASSISTANT, streaming = true, typing = true))
        }

        composeRule.onNodeWithTag(TYPING_DOTS_TEST_TAG).assertIsDisplayed()
    }

    @Test
    fun onceTheAnswerStartsTheDotsAreGone() {
        composeRule.setContent {
            Bubble(
                row(
                    kind = ConversationRowKind.ASSISTANT,
                    text = "Working on it",
                    streaming = true,
                    typing = false,
                ),
            )
        }

        composeRule.onNodeWithText("Working on it").assertIsDisplayed()
        composeRule.onNodeWithTag(TYPING_DOTS_TEST_TAG).assertDoesNotExist()
    }

    @androidx.compose.runtime.Composable
    private fun Bubble(row: ConversationRow) {
        ChatMessageBubble(
            row = row,
            enabled = true,
            onApproveTool = {},
            onRejectTool = { _, _ -> },
            onCancelTool = { _, _ -> },
            onAnswerTool = { _, _ -> },
            onRetry = {},
            onOpenLink = { _, _ -> },
            previewingRemotePath = "",
            previewLoading = false,
            modifier = Modifier,
        )
    }

    private fun row(
        kind: ConversationRowKind,
        text: String = "",
        blocks: List<MessageBlock> = emptyList(),
        streaming: Boolean = false,
        typing: Boolean = false,
        showRetry: Boolean = false,
    ) = ConversationRow(
        id = "row-1",
        kind = kind,
        text = text,
        thinking = null,
        images = emptyList(),
        tools = emptyList(),
        blocks = blocks,
        streaming = streaming,
        pending = false,
        typing = typing,
        showRetry = showRetry,
    )

    private fun runningTool(): ToolCard = ToolCard(
        id = "tool-1",
        name = "Read",
        phase = ToolPhase.RUNNING,
        kind = ToolKind.DOCUMENT,
        operation = ToolOperation.READ_FILE,
        target = "AndroidManifest.xml",
        filePath = "",
        fileLabel = "",
        input = "",
        output = "",
        question = null,
        actions = emptySet(),
    )

    private fun string(resource: Int): String =
        InstrumentationRegistry.getInstrumentation().targetContext.getString(resource)
}
