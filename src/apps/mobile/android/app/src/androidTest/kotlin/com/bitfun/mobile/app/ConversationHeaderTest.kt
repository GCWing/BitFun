package com.bitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import com.bitfun.mobile.app.ui.chat.CONVERSATION_MENU_TEST_TAG
import com.bitfun.mobile.app.ui.chat.CONVERSATION_RENAME_TEST_TAG
import com.bitfun.mobile.app.ui.chat.CONVERSATION_TITLE_TEST_TAG
import com.bitfun.mobile.app.ui.chat.ConversationHeader
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test

/**
 * The conversation header, ported from `RemoteChatHeader.ets`.
 *
 * What is worth pinning is the rename: it is the one destructive-ish thing the
 * header does, it is reached by tapping a label that otherwise looks inert, and
 * a stale draft surviving into another session would rename the wrong one.
 */
class ConversationHeaderTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun bothLinesAreShownAndAnUntitledSessionStillHasAName() {
        composeRule.setContent {
            ConversationHeader(
                title = "",
                contextTitle = "BitFun · main",
                canStop = false,
                enabled = true,
                onBack = {},
                onRename = {},
                onOpenSettings = {},
                onStop = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithText("Remote session").assertIsDisplayed()
        composeRule.onNodeWithText("BitFun · main").assertIsDisplayed()
    }

    @Test
    fun tappingTheTitleOpensTheEditorAndSavingSendsTheTrimmedTitle() {
        var renamed: String? = null

        composeRule.setContent {
            ConversationHeader(
                title = "Old title",
                contextTitle = "Studio",
                canStop = false,
                enabled = true,
                onBack = {},
                onRename = { renamed = it },
                onOpenSettings = {},
                onStop = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).assertDoesNotExist()
        composeRule.onNodeWithTag(CONVERSATION_TITLE_TEST_TAG).performClick()
        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).performTextClearance()
        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).performTextInput("  New title  ")
        composeRule.onNodeWithText("Save").performClick()

        assertEquals("New title", renamed)
        // The editor closes behind the save, so the header goes back to being a
        // header rather than staying a form over the transcript.
        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).assertDoesNotExist()
    }

    @Test
    fun anEmptyTitleCannotBeSavedAndCancelChangesNothing() {
        var renamed: String? = null

        composeRule.setContent {
            ConversationHeader(
                title = "Old title",
                contextTitle = "Studio",
                canStop = false,
                enabled = true,
                onBack = {},
                onRename = { renamed = it },
                onOpenSettings = {},
                onStop = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(CONVERSATION_TITLE_TEST_TAG).performClick()
        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).performTextClearance()
        composeRule.onNodeWithText("Save").assertIsNotEnabled()
        composeRule.onNodeWithText("Cancel").performClick()

        assertNull(renamed)
        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).assertDoesNotExist()
    }

    @Test
    fun theMenuOffersStopOnlyWhileATurnIsRunning() {
        var stopped = 0

        composeRule.setContent {
            ConversationHeader(
                title = "Session",
                contextTitle = "Studio",
                canStop = true,
                enabled = true,
                onBack = {},
                onRename = {},
                onOpenSettings = {},
                onStop = { stopped += 1 },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(CONVERSATION_MENU_TEST_TAG).performClick()
        composeRule.onNodeWithText("Session settings").assertIsDisplayed()
        composeRule.onNodeWithText("Stop").performClick()

        assertEquals(1, stopped)
    }

    @Test
    fun anIdleSessionHasNothingToStop() {
        composeRule.setContent {
            ConversationHeader(
                title = "Session",
                contextTitle = "Studio",
                canStop = false,
                enabled = true,
                onBack = {},
                onRename = {},
                onOpenSettings = {},
                onStop = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(CONVERSATION_MENU_TEST_TAG).performClick()

        composeRule.onNodeWithText("Session settings").assertIsDisplayed()
        composeRule.onNodeWithText("Stop").assertDoesNotExist()
    }
}
