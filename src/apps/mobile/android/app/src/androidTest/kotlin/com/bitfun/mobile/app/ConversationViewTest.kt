package com.bitfun.mobile.app

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.unit.dp
import androidx.test.platform.app.InstrumentationRegistry
import com.bitfun.mobile.app.ui.chat.CONVERSATION_LIST_TEST_TAG
import com.bitfun.mobile.app.ui.chat.CHAT_STATUS_DOT_TEST_TAG
import com.bitfun.mobile.app.ui.chat.CHAT_STATUS_BAR_TEST_TAG
import com.bitfun.mobile.app.ui.chat.ChatStatusBar
import com.bitfun.mobile.app.ui.chat.ConversationTimelineView
import com.bitfun.mobile.app.ui.theme.BitFunTheme
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.session.ConversationRow
import com.bitfun.mobile.core.feature.session.ConversationRowKind
import com.bitfun.mobile.core.feature.workspace.RemoteFileDownloadUiState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class ConversationViewTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun openingATallLastMessageStartsAtTheActualTail() {
        val answer = List(500) { "A long answer line." }.joinToString(" ") + " tail-marker"

        composeRule.setContent {
            BitFunTheme(dark = false) {
                ConversationTimelineView(
                    rows = listOf(assistantRow(answer)),
                    hasMoreMessages = false,
                    onLoadOlder = {},
                    enabled = true,
                    onApproveTool = {},
                    onRejectTool = { _, _ -> },
                    onCancelTool = { _, _ -> },
                    onAnswerTool = { _, _ -> },
                    onRetry = {},
                    onOpenFile = { _, _ -> },
                    previewingRemotePath = "",
                    previewLoading = false,
                    download = RemoteFileDownloadUiState.None,
                    onDownloadFile = { _, _ -> },
                    downloadEnabled = true,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }

        composeRule.waitForIdle()

        val listBounds = composeRule.onNodeWithTag(CONVERSATION_LIST_TEST_TAG)
            .getUnclippedBoundsInRoot()
        val answerBounds = composeRule.onNodeWithText("tail-marker", substring = true)
            .getUnclippedBoundsInRoot()
        assertTrue(answerBounds.bottom <= listBounds.bottom + 1.dp)
        composeRule.onNodeWithContentDescription(string(R.string.chat_scroll_to_bottom))
            .assertDoesNotExist()
    }

    @Test
    fun streamingGrowthKeepsFollowingWhileTheReaderIsAtTheTail() {
        val row = mutableStateOf(assistantRow("stream-start", streaming = true))

        composeRule.setContent {
            BitFunTheme(dark = false) {
                TimelineForTest(listOf(row.value))
            }
        }
        composeRule.runOnIdle {
            row.value = assistantRow(
                List(500) { "Streaming answer line." }.joinToString(" ") + " stream-tail-marker",
                streaming = true,
            )
        }
        composeRule.waitForIdle()

        val listBounds = composeRule.onNodeWithTag(CONVERSATION_LIST_TEST_TAG)
            .getUnclippedBoundsInRoot()
        val answerBounds = composeRule.onNodeWithText("stream-tail-marker", substring = true)
            .getUnclippedBoundsInRoot()
        assertTrue(answerBounds.bottom <= listBounds.bottom + 1.dp)
        composeRule.onNodeWithContentDescription(string(R.string.chat_scroll_to_bottom))
            .assertDoesNotExist()
    }

    @Test
    fun streamingGrowthDoesNotStealTheReaderAfterTheyLeaveTheTail() {
        val rows = mutableStateOf((1..40).map { index -> assistantRow("message-$index", id = "message-$index") })

        composeRule.setContent {
            BitFunTheme(dark = false) {
                TimelineForTest(rows.value)
            }
        }
        composeRule.waitForIdle()
        composeRule.onNodeWithTag(CONVERSATION_LIST_TEST_TAG).performTouchInput {
            swipeDown()
            swipeDown()
        }
        composeRule.waitForIdle()
        composeRule.onNodeWithContentDescription(string(R.string.chat_scroll_to_bottom)).assertIsDisplayed()

        composeRule.runOnIdle {
            rows.value = rows.value.dropLast(1) +
                assistantRow(
                    List(400) { "Growing final answer." }.joinToString(" ") + " reader-tail-marker",
                    id = "message-40",
                    streaming = true,
                )
        }
        composeRule.waitForIdle()

        composeRule.onNodeWithContentDescription(string(R.string.chat_scroll_to_bottom))
            .assertIsDisplayed()
            .performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText("reader-tail-marker", substring = true).assertIsDisplayed()
        composeRule.onNodeWithContentDescription(string(R.string.chat_scroll_to_bottom))
            .assertDoesNotExist()
    }

    @Test
    fun reconnectingStatusBarMatchesTheFixedHeightColorAndCopyContract() {
        composeRule.setContent {
            BitFunTheme(dark = false) {
                ChatStatusBar(
                    phase = ConnectionPhase.RECONNECTING,
                    canStop = false,
                    onStop = {},
                )
            }
        }

        val title = string(R.string.chat_status_restoring_connection)
        val detail = string(R.string.connection_reconnecting_desktop)
        composeRule.onNodeWithText("$title · $detail").assertExists()
        val bounds = composeRule.onNodeWithTag(CHAT_STATUS_BAR_TEST_TAG).getUnclippedBoundsInRoot()
        assertTrue(kotlin.math.abs((bounds.bottom - bounds.top).value - 48f) < 1f)
        val dot = composeRule.onNodeWithTag(CHAT_STATUS_DOT_TEST_TAG).captureToImage()
        assertEquals(0xFF706F6A.toInt(), dot.toPixelMap()[dot.width / 2, dot.height / 2].toArgb())
    }

    @Test
    fun executingStatusBarDoesNotAppendAConnectionDetail() {
        composeRule.setContent {
            BitFunTheme(dark = false) {
                ChatStatusBar(
                    phase = ConnectionPhase.RECONNECTING,
                    canStop = true,
                    onStop = {},
                )
            }
        }

        composeRule.onNodeWithText(string(R.string.chat_status_executing)).assertExists()
    }

    @Composable
    private fun TimelineForTest(rows: List<ConversationRow>) {
        ConversationTimelineView(
            rows = rows,
            hasMoreMessages = false,
            onLoadOlder = {},
            enabled = true,
            onApproveTool = {},
            onRejectTool = { _, _ -> },
            onCancelTool = { _, _ -> },
            onAnswerTool = { _, _ -> },
            onRetry = {},
            onOpenFile = { _, _ -> },
            previewingRemotePath = "",
            previewLoading = false,
            download = RemoteFileDownloadUiState.None,
            onDownloadFile = { _, _ -> },
            downloadEnabled = true,
            modifier = Modifier.fillMaxSize(),
        )
    }

    private fun assistantRow(
        answer: String,
        id: String = "message-1",
        streaming: Boolean = false,
    ): ConversationRow = ConversationRow(
        id = id,
        kind = ConversationRowKind.ASSISTANT,
        text = answer,
        thinking = null,
        images = emptyList(),
        tools = emptyList(),
        blocks = emptyList(),
        streaming = streaming,
        typing = false,
        pending = false,
        showRetry = false,
    )

    private fun string(resource: Int): String =
        InstrumentationRegistry.getInstrumentation().targetContext.getString(resource)
}
