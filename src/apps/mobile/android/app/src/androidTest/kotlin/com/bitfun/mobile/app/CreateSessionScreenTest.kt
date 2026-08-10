package com.bitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import com.bitfun.mobile.app.ui.chat.COMPOSER_INPUT_TEST_TAG
import com.bitfun.mobile.app.ui.chat.COMPOSER_SEND_TEST_TAG
import com.bitfun.mobile.app.ui.remote.CREATE_SESSION_BACK_TEST_TAG
import com.bitfun.mobile.app.ui.remote.CREATE_SESSION_WORKSPACE_TEST_TAG
import com.bitfun.mobile.app.ui.remote.CreateSessionScreen
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.session.RemoteSessionIntent
import com.bitfun.mobile.core.feature.workspace.RemoteFilePreviewUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test

/**
 * The new-session screen, ported from `RemoteCreateSessionView.ets`.
 *
 * What is worth pinning is what leaves the phone: the screen has no title field
 * and no agent picker, so everything about the created session is inferred, and
 * a wrong inference produces a session bound to the wrong place.
 *
 * The workspace-picked path is covered by `CreateSessionPresentationTest` in
 * core-feature, where that decision actually lives.
 */
class CreateSessionScreenTest {
    @get:Rule
    val composeRule = createComposeRule()

    private fun readyWorkspace(): RemoteWorkspaceUiState = RemoteWorkspaceUiState.Ready(
        workspaces = emptyList(),
        assistants = emptyList(),
        selected = null,
        preview = RemoteFilePreviewUiState.None,
        busy = false,
    )

    @Test
    fun withoutAWorkspaceTheScreenSaysChatAndCreatesAChatSession() {
        var intent: RemoteSessionIntent? = null

        composeRule.setContent {
            CreateSessionScreen(
                workspaceState = readyWorkspace(),
                phase = ConnectionPhase.CONNECTED,
                deviceId = "desk-1",
                busy = false,
                onBack = {},
                onWorkspaceIntent = {},
                onIntent = { intent = it },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithText("Chat").assertIsDisplayed()
        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).performTextInput("review the parser")
        composeRule.onNodeWithTag(COMPOSER_SEND_TEST_TAG).performClick()

        val create = intent as RemoteSessionIntent.CreateSession
        // "Claw" is a wire value the desktop matches on, not a label.
        assertEquals("Claw", create.agentType)
        assertEquals("review the parser", create.instruction)
        // The agent renames the session after reading the first message, so
        // sending a title here would only be overwritten.
        assertEquals("", create.title)
    }

    @Test
    fun withoutADesktopThereIsNothingToCreateOn() {
        var intent: RemoteSessionIntent? = null

        composeRule.setContent {
            CreateSessionScreen(
                workspaceState = readyWorkspace(),
                phase = ConnectionPhase.CONNECTED,
                deviceId = "",
                busy = false,
                onBack = {},
                onWorkspaceIntent = {},
                onIntent = { intent = it },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithText("No desktop is connected.").assertIsDisplayed()
        // The draft survives — it is worth keeping until a desktop comes back —
        // but the send is the same dimmed control a dropout produces.
        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).performTextInput("review the parser")
        composeRule.onNodeWithTag(COMPOSER_SEND_TEST_TAG).performClick()

        assertNull(intent)
    }

    @Test
    fun theWorkspaceRowOpensAPickerThatSaysWhenThereIsNothingToPick() {
        composeRule.setContent {
            CreateSessionScreen(
                workspaceState = readyWorkspace(),
                phase = ConnectionPhase.CONNECTED,
                deviceId = "desk-1",
                busy = false,
                onBack = {},
                onWorkspaceIntent = {},
                onIntent = {},
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(CREATE_SESSION_WORKSPACE_TEST_TAG).performClick()

        composeRule.onNodeWithText("Choose a workspace").assertIsDisplayed()
        // A desktop with no recent projects is not a broken picker, and saying
        // where to fix it is the only useful thing the sheet can offer.
        composeRule.onNodeWithText("No recent workspaces. Open one on the desktop first.")
            .assertIsDisplayed()
    }

    @Test
    fun backLeavesWithoutCreatingAnything() {
        var back = 0
        var intent: RemoteSessionIntent? = null

        composeRule.setContent {
            CreateSessionScreen(
                workspaceState = readyWorkspace(),
                phase = ConnectionPhase.CONNECTED,
                deviceId = "desk-1",
                busy = false,
                onBack = { back += 1 },
                onWorkspaceIntent = {},
                onIntent = { intent = it },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).performTextInput("never mind")
        composeRule.onNodeWithTag(CREATE_SESSION_BACK_TEST_TAG).performClick()

        assertEquals(1, back)
        assertNull(intent)
    }
}
