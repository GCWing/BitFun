package com.bitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.bitfun.mobile.app.ui.remote.RemoteSessionListView
import com.bitfun.mobile.app.ui.remote.SESSION_SEARCH_FIELD_TEST_TAG
import com.bitfun.mobile.app.ui.remote.SESSION_SEARCH_TOGGLE_TEST_TAG
import com.bitfun.mobile.app.ui.settings.VIEW_SETTINGS_TEST_TAG
import com.bitfun.mobile.app.ui.settings.VIEW_SETTINGS_TOGGLE_TEST_TAG
import com.bitfun.mobile.app.ui.theme.BitFunTheme
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import org.junit.Rule
import org.junit.Test

class RemoteSessionListViewTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun remoteListUsesTheCompactSidebarHeader() {
        composeRule.setContent {
            BitFunTheme(dark = false) {
                RemoteSessionListView(
                    state = RemoteSessionUiState.Idle,
                    workspaceState = RemoteWorkspaceUiState.Idle,
                    connectionDetails = {},
                    onIntent = {},
                    onWorkspaceIntent = {},
                    onOpen = {},
                    onCreate = {},
                    modifier = Modifier,
                )
            }
        }

        composeRule.onNodeWithText("BitFun").assertIsDisplayed()
        composeRule.onNodeWithTag(VIEW_SETTINGS_TOGGLE_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(SESSION_SEARCH_TOGGLE_TEST_TAG).assertIsDisplayed()
        composeRule.onAllNodesWithText("Sessions").assertCountEquals(0)
        composeRule.onAllNodesWithText("All").assertCountEquals(0)

        composeRule.onNodeWithTag(SESSION_SEARCH_FIELD_TEST_TAG).assertDoesNotExist()
        composeRule.onNodeWithTag(SESSION_SEARCH_TOGGLE_TEST_TAG).performClick()
        composeRule.onNodeWithTag(SESSION_SEARCH_FIELD_TEST_TAG).assertIsDisplayed()
    }

    @Test
    fun viewSettingsOpensAsASheetInsteadOfExpandingTheList() {
        composeRule.setContent {
            BitFunTheme(dark = false) {
                RemoteSessionListView(
                    state = RemoteSessionUiState.Ready(
                        sessions = emptyList(),
                        selectedSessionId = null,
                        timeline = null,
                        busy = false,
                        permissionMode = null,
                        permissionModeFailure = null,
                        query = "",
                        agentFilter = com.bitfun.mobile.core.feature.session.SessionAgentFilter.ALL,
                        hasMore = false,
                    ),
                    workspaceState = RemoteWorkspaceUiState.Idle,
                    connectionDetails = {},
                    onIntent = {},
                    onWorkspaceIntent = {},
                    onOpen = {},
                    onCreate = {},
                    modifier = Modifier,
                )
            }
        }

        composeRule.onNodeWithTag(VIEW_SETTINGS_TEST_TAG).assertDoesNotExist()
        composeRule.onNodeWithTag(VIEW_SETTINGS_TOGGLE_TEST_TAG).performClick()
        composeRule.onNodeWithTag(VIEW_SETTINGS_TEST_TAG).assertIsDisplayed()
    }
}
