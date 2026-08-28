package com.bitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.bitfun.mobile.app.ui.chat.tool.TOOL_EXPAND_TEST_TAG
import androidx.compose.ui.test.performTextInput
import com.bitfun.mobile.app.ui.chat.tool.ToolStatusList
import com.bitfun.mobile.app.ui.chat.tool.ToolStatusRow
import com.bitfun.mobile.core.feature.session.ToolAction
import com.bitfun.mobile.core.feature.session.ToolCard
import com.bitfun.mobile.core.feature.session.ToolKind
import com.bitfun.mobile.core.feature.session.ToolOperation
import com.bitfun.mobile.core.feature.session.ToolPhase
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

/**
 * A tool row and the blocking panels that hang off it, ported from
 * `ToolStatusList.ets` and `ToolInteractionPanels.ets`.
 *
 * These need no relay: a row is a pure function of a [ToolCard]. What is worth
 * pinning is that a turn waiting on the user cannot be dismissed into a state
 * where nothing on screen says so — and that a finished tool says what it did
 * rather than only that it is done.
 */
class ToolStatusRowTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun approvalOffersBothVerdictsAndReportsTheOneTapped() {
        var approved = false
        var rejection: String? = null

        composeRule.setContent {
            ToolStatusRow(
                tool = pendingTool(setOf(ToolAction.APPROVE, ToolAction.REJECT)),
                enabled = true,
                onApprove = { approved = true },
                onReject = { rejection = it },
                onCancel = {},
                onAnswer = {},
                onAnswerStructured = {},
                onOpenFile = { _, _ -> },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithText("Approve").assertIsDisplayed()
        composeRule.onNodeWithText("Reject").performClick()

        assertEquals(false, approved)
        // The desktop refuses a rejection without a reason, so the client
        // supplies one rather than asking the user to type it.
        assertEquals("Rejected from the Android client", rejection)
    }

    @Test
    fun aQuestionAsksInlineAndOnlySendsOnceAnswered() {
        var answer: String? = null

        composeRule.setContent {
            ToolStatusRow(
                tool = pendingTool(setOf(ToolAction.ANSWER)).copy(
                    phase = ToolPhase.RUNNING,
                    question = "Which branch should this land on?",
                ),
                enabled = true,
                onApprove = {},
                onReject = {},
                onCancel = {},
                onAnswer = { answer = it },
                onAnswerStructured = {},
                onOpenFile = { _, _ -> },
                modifier = Modifier,
            )
        }

        // No tap needed to reach it: the panel is the transcript, not a dialog.
        composeRule.onNodeWithText("Which branch should this land on?").assertIsDisplayed()
        composeRule.onNodeWithText("Send answer").assertIsNotEnabled()

        composeRule.onNodeWithText("Your answer").performTextInput("  main  ")
        composeRule.onNodeWithText("Send answer").assertIsEnabled().performClick()

        assertEquals("main", answer)
    }

    @Test
    fun aDisconnectedRowShowsItsActionsWithoutOfferingThem() {
        var approved = false

        composeRule.setContent {
            ToolStatusRow(
                tool = pendingTool(setOf(ToolAction.APPROVE, ToolAction.REJECT)),
                enabled = false,
                onApprove = { approved = true },
                onReject = {},
                onCancel = {},
                onAnswer = {},
                onAnswerStructured = {},
                onOpenFile = { _, _ -> },
                modifier = Modifier,
            )
        }

        // Kept on screen rather than hidden: the turn is still blocked on this
        // tool, and a row that loses its buttons reads as one that resolved.
        composeRule.onNodeWithText("Approve").assertIsNotEnabled()
        assertEquals(false, approved)
    }

    @Test
    fun aRunningToolLeadsWithItsTargetAndOffersOnlyTheStop() {
        var cancellation: String? = null

        composeRule.setContent {
            ToolStatusRow(
                tool = pendingTool(setOf(ToolAction.CANCEL)).copy(phase = ToolPhase.RUNNING),
                enabled = true,
                onApprove = {},
                onReject = {},
                onCancel = { cancellation = it },
                onAnswer = {},
                onAnswerStructured = {},
                onOpenFile = { _, _ -> },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithText("Running \"README.md\"").assertIsDisplayed()
        composeRule.onNodeWithText("Stop").performClick()

        assertEquals("Cancelled from the Android client", cancellation)
    }

    @Test
    fun aFinishedToolSaysWhatItDidAndHasNothingToActOn() {
        composeRule.setContent {
            ToolStatusRow(
                tool = pendingTool(emptySet()).copy(phase = ToolPhase.COMPLETED),
                enabled = true,
                onApprove = {},
                onReject = {},
                onCancel = {},
                onAnswer = {},
                onAnswerStructured = {},
                onOpenFile = { _, _ -> },
                modifier = Modifier,
            )
        }

        // The state is carried by the badge, so the line is free to say the one
        // thing a chip reading "Done" never did: which file was edited.
        composeRule.onNodeWithText("Edit file · README.md").assertIsDisplayed()
        composeRule.onNodeWithTag(TOOL_EXPAND_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithText("Approve").assertDoesNotExist()
        composeRule.onNodeWithText("Your answer").assertDoesNotExist()
    }

    @Test
    fun aCompletedEmptyPreviewStillExpandsWithChevronFeedback() {
        composeRule.setContent {
            ToolStatusRow(
                tool = readTool("a", "One.kt"),
                enabled = true,
                onApprove = {},
                onReject = {},
                onCancel = {},
                onAnswer = {},
                onAnswerStructured = {},
                onOpenFile = { _, _ -> },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(TOOL_EXPAND_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(TOOL_EXPAND_TEST_TAG).performClick()
        composeRule.onNodeWithTag(TOOL_EXPAND_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithText("Read file · One.kt").assertIsDisplayed()
    }

    @Test
    fun tappingAFileRowOpensThatFileRatherThanExpandingIt() {
        var opened: Pair<String, String>? = null

        composeRule.setContent {
            ToolStatusRow(
                tool = pendingTool(emptySet()).copy(
                    phase = ToolPhase.COMPLETED,
                    filePath = "docs/README.md",
                    fileLabel = "README.md",
                ),
                enabled = true,
                onApprove = {},
                onReject = {},
                onCancel = {},
                onAnswer = {},
                onAnswerStructured = {},
                onOpenFile = { path, label -> opened = path to label },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithText("Edit file · README.md").performClick()

        assertEquals("docs/README.md" to "README.md", opened)
    }

    @Test
    fun consecutiveFinishedLookupsFoldIntoOneLineUntilOpened() {
        composeRule.setContent {
            ToolStatusList(
                tools = listOf(readTool("a", "One.kt"), readTool("b", "Two.kt")),
                enabled = true,
                onApprove = {},
                onReject = { _, _ -> },
                onCancel = { _, _ -> },
                onAnswer = { _, _ -> },
                onAnswerStructured = { _, _ -> },
                onOpenFile = { _, _ -> },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithText("Ran 2 tools").assertIsDisplayed()
        composeRule.onNodeWithText("Read file · One.kt").assertDoesNotExist()

        composeRule.onNodeWithText("Ran 2 tools").performClick()

        composeRule.onNodeWithText("Read file · One.kt").assertIsDisplayed()
        composeRule.onNodeWithText("Read file · Two.kt").assertIsDisplayed()
    }

    private fun pendingTool(actions: Set<ToolAction>): ToolCard = ToolCard(
        id = "tool-1",
        name = "Edit",
        phase = ToolPhase.PENDING_CONFIRMATION,
        kind = ToolKind.MUTATE,
        operation = ToolOperation.EDIT_FILE,
        target = "README.md",
        filePath = "",
        fileLabel = "",
        input = "{\"path\": \"README.md\"}",
        output = "",
        question = null,
        actions = actions,
    )

    private fun readTool(id: String, target: String): ToolCard = ToolCard(
        id = id,
        name = "Read",
        phase = ToolPhase.COMPLETED,
        kind = ToolKind.DOCUMENT,
        operation = ToolOperation.READ_FILE,
        target = target,
        filePath = "",
        fileLabel = "",
        input = "",
        output = "",
        question = null,
        actions = emptySet(),
    )
}
