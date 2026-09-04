package com.openbitfun.mobile.app

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.unit.dp
import androidx.test.platform.app.InstrumentationRegistry
import com.openbitfun.mobile.app.ui.chat.CONVERSATION_LIST_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.CONVERSATION_LOADING_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.CHAT_STATUS_DOT_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.CHAT_STATUS_BAR_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.ChatStatusBar
import com.openbitfun.mobile.app.ui.chat.ConversationEmptyState
import com.openbitfun.mobile.app.ui.chat.ConversationTimelineView
import com.openbitfun.mobile.app.ui.chat.COMPOSER_INPUT_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.COMPOSER_SEND_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.ConversationView
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import com.openbitfun.mobile.core.feature.connection.ConnectionPhase
import com.openbitfun.mobile.core.feature.layout.SettingsPlacement
import com.openbitfun.mobile.core.feature.layout.SettingsPlacementMode
import com.openbitfun.mobile.core.feature.session.ConversationRow
import com.openbitfun.mobile.core.feature.session.ConversationRowKind
import com.openbitfun.mobile.core.feature.session.RemoteSessionIntent
import com.openbitfun.mobile.core.feature.session.RemoteSessionUiState
import com.openbitfun.mobile.core.feature.session.SessionAgentFilter
import com.openbitfun.mobile.core.feature.workspace.RemoteFileDownloadUiState
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
            OpenBitFunTheme(dark = false) {
                ConversationTimelineView(
                    rows = listOf(assistantRow(answer)),
                    hasMoreMessages = false,
                    onLoadOlder = {},
                    enabled = true,
                    onApproveTool = {},
                    onRejectTool = { _, _ -> },
                    onCancelTool = { _, _ -> },
                    onAnswerTool = { _, _ -> },
                    onAnswerToolStructured = { _, _ -> },
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
            OpenBitFunTheme(dark = false) {
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
            OpenBitFunTheme(dark = false) {
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
    fun withLoadOlderHeaderStreamingGrowthStaysOnTheRealTail() {
        val rows = mutableStateOf(
            (1..40).map { index -> assistantRow("message-$index", id = "message-$index") },
        )

        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                TimelineForTest(rows.value, hasMoreMessages = true)
            }
        }
        composeRule.waitForIdle()

        composeRule.runOnIdle {
            rows.value = rows.value.dropLast(1) +
                assistantRow(
                    List(500) { "Growing tail line." }.joinToString(" ") + " header-tail-marker",
                    id = "message-40",
                    streaming = true,
                )
        }
        composeRule.waitForIdle()

        val listBounds = composeRule.onNodeWithTag(CONVERSATION_LIST_TEST_TAG)
            .getUnclippedBoundsInRoot()
        val tailBounds = composeRule.onNodeWithText("header-tail-marker", substring = true)
            .getUnclippedBoundsInRoot()
        assertTrue(tailBounds.bottom <= listBounds.bottom + 1.dp)
        composeRule.onNodeWithContentDescription(string(R.string.chat_scroll_to_bottom))
            .assertDoesNotExist()
    }

    @Test
    fun conversationWithNoTimelineShowsLoadingStateInsteadOfBlankSurface() {
        setConversationContent(state = { readyState() })

        composeRule.onNodeWithTag(CONVERSATION_LOADING_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithText(string(R.string.chat_empty_loading)).assertIsDisplayed()
    }

    @Test
    fun composerShowsTheStoreDraftAndTypingDispatchesUpdateDraft() {
        val intents = mutableListOf<RemoteSessionIntent>()
        val state = mutableStateOf(readyState(sessionId = "s-code", draft = "existing draft"))

        setConversationContent(state = { state.value }, onIntent = { intents += it })

        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).assertTextEquals("existing draft")
        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).performTextReplacement("replaced draft")

        assertEquals(
            listOf<RemoteSessionIntent>(RemoteSessionIntent.UpdateDraft("replaced draft")),
            intents,
        )
    }

    @Test
    fun composerFollowsStoreDraftUpdatesWithinTheSameSession() {
        val state = mutableStateOf(readyState(sessionId = "s-code", draft = "first"))

        setConversationContent(state = { state.value })

        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).assertTextEquals("first")
        composeRule.runOnIdle { state.value = state.value.copy(draft = "second") }
        composeRule.waitForIdle()
        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).assertTextEquals("second")
    }

    @Test
    fun switchingSessionsShowsTheRestoredDraft() {
        val state = mutableStateOf(readyState(sessionId = "s-a", draft = "draft-a"))

        setConversationContent(state = { state.value })

        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).assertTextEquals("draft-a")
        composeRule.runOnIdle {
            state.value = readyState(sessionId = "s-b", draft = "draft-b")
        }
        composeRule.waitForIdle()
        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).assertTextEquals("draft-b")
    }

    @Test
    fun sendUsesTheStoreDraftAndDoesNotFakeClearIt() {
        val intents = mutableListOf<RemoteSessionIntent>()

        setConversationContent(
            state = { readyState(sessionId = "s-code", draft = "send me") },
            onIntent = { intents += it },
        )

        composeRule.onNodeWithTag(COMPOSER_SEND_TEST_TAG).performClick()

        assertEquals(
            listOf<RemoteSessionIntent>(RemoteSessionIntent.SendMessage("s-code", "send me", null)),
            intents,
        )
    }

    @Test
    fun emptyStateShowsInvitationCopy() {
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                ConversationEmptyState(modifier = Modifier.fillMaxSize())
            }
        }
        composeRule.onNodeWithText(string(R.string.chat_empty_title)).assertIsDisplayed()
        composeRule.onNodeWithText(string(R.string.chat_empty_hint)).assertIsDisplayed()
    }

    @Test
    fun reconnectingStatusBarMatchesTheFixedHeightColorAndCopyContract() {
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
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
            OpenBitFunTheme(dark = false) {
                ChatStatusBar(
                    phase = ConnectionPhase.RECONNECTING,
                    canStop = true,
                    onStop = {},
                )
            }
        }

        composeRule.onNodeWithText(string(R.string.chat_status_executing)).assertExists()
    }

    private fun setConversationContent(
        state: () -> RemoteSessionUiState.Ready,
        onIntent: (RemoteSessionIntent) -> Unit = {},
    ) {
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                ConversationView(
                    state = state(),
                    phase = ConnectionPhase.CONNECTED,
                    settingsPlacement = SettingsPlacement(SettingsPlacementMode.BOTTOM, 0, 0, 0),
                    onBack = {},
                    onIntent = onIntent,
                    contextTitle = "Test desktop",
                    onOpenFile = { _, _ -> },
                    previewingRemotePath = "",
                    previewLoading = false,
                    download = RemoteFileDownloadUiState.None,
                    onDownloadFile = { _, _ -> },
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
    }

    private fun readyState(
        sessionId: String = "",
        draft: String = "",
    ) = RemoteSessionUiState.Ready(
        sessions = emptyList(),
        selectedSessionId = sessionId,
        timeline = null,
        busy = false,
        permissionMode = null,
        permissionModeFailure = null,
        query = "",
        agentFilter = SessionAgentFilter.ALL,
        hasMore = false,
        hasMoreMessages = false,
        modelCatalog = null,
        modelCatalogFailure = null,
        draft = draft,
    )

    @Composable
    private fun TimelineForTest(rows: List<ConversationRow>, hasMoreMessages: Boolean = false) {
        ConversationTimelineView(
            rows = rows,
            hasMoreMessages = hasMoreMessages,
            onLoadOlder = {},
            enabled = true,
            onApproveTool = {},
            onRejectTool = { _, _ -> },
            onCancelTool = { _, _ -> },
            onAnswerTool = { _, _ -> },
            onAnswerToolStructured = { _, _ -> },
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
