package com.openbitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.openbitfun.mobile.app.ui.remote.ChatCreateControl
import com.openbitfun.mobile.app.ui.remote.ProjectCreateControl
import com.openbitfun.mobile.app.ui.remote.RemoteSessionListView
import com.openbitfun.mobile.app.ui.remote.SESSION_CHAT_CREATE_TEST_TAG
import com.openbitfun.mobile.app.ui.remote.SESSION_PROJECT_CREATE_TEST_TAG_PREFIX
import com.openbitfun.mobile.app.ui.remote.SESSION_SEARCH_FIELD_TEST_TAG
import com.openbitfun.mobile.app.ui.remote.SESSION_SEARCH_TOGGLE_TEST_TAG
import com.openbitfun.mobile.app.ui.settings.VIEW_SETTINGS_TEST_TAG
import com.openbitfun.mobile.app.ui.settings.VIEW_SETTINGS_TOGGLE_TEST_TAG
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import com.openbitfun.mobile.core.feature.session.RemoteSessionUiState
import com.openbitfun.mobile.core.feature.layout.SettingsPlacement
import com.openbitfun.mobile.core.feature.layout.SettingsPlacementMode
import com.openbitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

class RemoteSessionListViewTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun remoteListUsesTheCompactSidebarHeader() {
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                RemoteSessionListView(
                    state = RemoteSessionUiState.Idle,
                    workspaceState = RemoteWorkspaceUiState.Idle,
                    compact = true,
                    sessionDetailsPlacement = compactPlacement,
                    viewSettingsPlacement = compactPlacement,
                    connectionDetails = {},
                    onIntent = {},
                    onWorkspaceIntent = {},
                    onOpen = {},
                    onCreate = {},
                    modifier = Modifier,
                )
            }
        }

        composeRule.onNodeWithText("OpenBitFun").assertIsDisplayed()
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
            OpenBitFunTheme(dark = false) {
                RemoteSessionListView(
                    state = RemoteSessionUiState.Ready(
                        sessions = emptyList(),
                        selectedSessionId = null,
                        timeline = null,
                        busy = false,
                        permissionMode = null,
                        permissionModeFailure = null,
                        query = "",
                        agentFilter = com.openbitfun.mobile.core.feature.session.SessionAgentFilter.ALL,
                        hasMore = false,
                        hasMoreMessages = false,
                        modelCatalog = null,
                    ),
                    workspaceState = RemoteWorkspaceUiState.Idle,
                    compact = true,
                    sessionDetailsPlacement = compactPlacement,
                    viewSettingsPlacement = compactPlacement,
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

    @Test
    fun projectCreateUsesAnAnchoredCodeCoworkMenu() {
        val open = mutableStateOf(false)
        var agentType: String? = null
        val path = "/work/openbitfun"

        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                ProjectCreateControl(
                    path = path,
                    expanded = open.value,
                    onToggle = { open.value = !open.value },
                    onDismiss = { open.value = false },
                    onCreateAgent = { agentType = it },
                )
            }
        }

        composeRule.onNodeWithTag(SESSION_PROJECT_CREATE_TEST_TAG_PREFIX + path).performClick()
        composeRule.onNodeWithText("Code").assertIsDisplayed().performClick()
        assertEquals("code", agentType)
    }

    @Test
    fun chatSectionCreateIsASectionAnchoredAction() {
        var clicked = false
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                ChatCreateControl(onCreate = { clicked = true })
            }
        }

        composeRule.onNodeWithTag(SESSION_CHAT_CREATE_TEST_TAG).assertIsDisplayed().performClick()
        assertEquals(true, clicked)
    }
}

private val compactPlacement = SettingsPlacement(
    mode = SettingsPlacementMode.BOTTOM,
    width = 0,
    height = 0,
    maxHeight = 0,
)
