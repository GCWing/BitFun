package com.bitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.test.platform.app.InstrumentationRegistry
import com.bitfun.mobile.app.ui.chat.CONVERSATION_MENU_TEST_TAG
import com.bitfun.mobile.app.ui.chat.CONVERSATION_RENAME_TEST_TAG
import com.bitfun.mobile.app.ui.chat.CONVERSATION_TITLE_TEST_TAG
import com.bitfun.mobile.app.ui.chat.ConversationHeader
import com.bitfun.mobile.app.ui.chat.HEADER_ACTION_MENU_TEST_TAG
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
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
                onStop = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithText(string(R.string.conversation_title_default)).assertIsDisplayed()
        composeRule.onNodeWithText("BitFun · main").assertIsDisplayed()
    }

    @Test
    fun aCompactConversationOpensTheSidebarInsteadOfNavigatingBack() {
        var sidebarOpens = 0

        composeRule.setContent {
            ConversationHeader(
                title = "Session",
                contextTitle = "Studio",
                canStop = false,
                enabled = true,
                onBack = { error("Compact conversation must not navigate back") },
                onOpenSidebar = { sidebarOpens += 1 },
                onRename = {},
                onStop = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithContentDescription(string(R.string.shell_open_sidebar)).performClick()
        composeRule.onNodeWithContentDescription(string(R.string.conversation_back)).assertDoesNotExist()
        assertEquals(1, sidebarOpens)
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
                onStop = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).assertDoesNotExist()
        composeRule.onNodeWithTag(CONVERSATION_TITLE_TEST_TAG).performClick()
        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).performTextClearance()
        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).performTextInput("  New title  ")
        composeRule.onNodeWithText(string(R.string.session_rename_confirm)).performClick()

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
                onStop = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(CONVERSATION_TITLE_TEST_TAG).performClick()
        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).performTextClearance()
        composeRule.onNodeWithText(string(R.string.session_rename_confirm)).assertIsNotEnabled()
        composeRule.onNodeWithText(string(R.string.common_cancel)).performClick()

        assertNull(renamed)
        composeRule.onNodeWithTag(CONVERSATION_RENAME_TEST_TAG).assertDoesNotExist()
    }

    @Test
    fun theMenuOffersUploadedFilesAndStopOnlyWhileATurnIsRunning() {
        var stopped = 0

        composeRule.setContent {
            ConversationHeader(
                title = "Session",
                contextTitle = "Studio",
                canStop = true,
                enabled = true,
                onBack = {},
                onRename = {},
                onStop = { stopped += 1 },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(CONVERSATION_MENU_TEST_TAG).performClick()
        composeRule.onNodeWithText(string(R.string.session_uploaded_files)).assertIsDisplayed()
        val menuBounds = composeRule.onNodeWithTag(HEADER_ACTION_MENU_TEST_TAG).getUnclippedBoundsInRoot()
        assertTrue(kotlin.math.abs((menuBounds.right - menuBounds.left).value - 292f) < 1f)
        assertTrue(kotlin.math.abs((menuBounds.bottom - menuBounds.top).value - 161f) < 1f)
        composeRule.onNodeWithText(string(R.string.message_stop)).performClick()

        assertEquals(1, stopped)
    }

    @Test
    fun tappingTheMenuAgainDismissesIt() {
        composeRule.setContent {
            ConversationHeader(
                title = "Session",
                contextTitle = "Studio",
                canStop = false,
                enabled = true,
                onBack = {},
                onRename = {},
                onStop = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(CONVERSATION_MENU_TEST_TAG).performClick()
        composeRule.onNodeWithTag(HEADER_ACTION_MENU_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(CONVERSATION_MENU_TEST_TAG).performClick()
        composeRule.onNodeWithTag(HEADER_ACTION_MENU_TEST_TAG).assertDoesNotExist()
    }

    @Test
    fun anIdleSessionStillOffersUploadedFilesButNotStop() {
        composeRule.setContent {
            ConversationHeader(
                title = "Session",
                contextTitle = "Studio",
                canStop = false,
                enabled = true,
                onBack = {},
                onRename = {},
                onStop = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(CONVERSATION_MENU_TEST_TAG).performClick()

        composeRule.onNodeWithText(string(R.string.session_uploaded_files)).assertIsDisplayed()
        composeRule.onNodeWithText(string(R.string.message_stop)).assertDoesNotExist()
    }

    private fun string(resource: Int): String =
        InstrumentationRegistry.getInstrumentation().targetContext.getString(resource)
}
