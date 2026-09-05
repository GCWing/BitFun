package com.openbitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.openbitfun.mobile.app.ui.settings.FULL_ACCESS_CONFIRM_TEST_TAG
import com.openbitfun.mobile.app.ui.settings.PermissionSection
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import com.openbitfun.mobile.core.feature.session.RemoteSessionIntent
import com.openbitfun.mobile.core.feature.session.RemoteSessionUiState
import com.openbitfun.mobile.core.feature.session.SessionAgentFilter
import com.openbitfun.mobile.core.feature.session.SessionPermissionMode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class PermissionModeCardTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun knownModesRenderAndSelectingAutoDispatches() {
        val intents = mutableListOf<RemoteSessionIntent>()
        setPermissionContent(
            permissionMode = SessionPermissionMode.ASK,
            onIntent = { intents += it },
        )

        composeRule.onNodeWithText("Ask first").assertIsDisplayed()
        composeRule.onNodeWithText("Approve automatically").assertIsDisplayed()
        composeRule.onNodeWithText("Full access").assertIsDisplayed()
        composeRule.onNodeWithText("Approve automatically").performClick()

        assertEquals(
            listOf(RemoteSessionIntent.SetPermissionMode(SessionPermissionMode.AUTO)),
            intents,
        )
    }

    @Test
    fun fullAccessRequiresConfirmationAndCanBeCancelled() {
        val intents = mutableListOf<RemoteSessionIntent>()
        setPermissionContent(
            permissionMode = SessionPermissionMode.ASK,
            onIntent = { intents += it },
        )

        composeRule.onNodeWithText("Full access").performClick()
        assertTrue(intents.isEmpty())
        composeRule.onNodeWithTag(FULL_ACCESS_CONFIRM_TEST_TAG).assertIsDisplayed()

        composeRule.onNodeWithText("Cancel").performClick()
        composeRule.onNodeWithTag(FULL_ACCESS_CONFIRM_TEST_TAG).assertDoesNotExist()
        assertTrue(intents.isEmpty())

        composeRule.onNodeWithText("Full access").performClick()
        composeRule.onNodeWithText("Turn on full access").performClick()
        assertEquals(
            listOf(RemoteSessionIntent.SetPermissionMode(SessionPermissionMode.FULL_ACCESS)),
            intents,
        )
    }

    @Test
    fun unknownModeExplainsFailureDisablesModesAndLeavesRefreshEnabled() {
        setPermissionContent(permissionMode = SessionPermissionMode.UNKNOWN)

        composeRule
            .onNodeWithText("The desktop's permission mode could not be read. Refresh to try again.")
            .assertIsDisplayed()
        composeRule.onNodeWithText("Ask first").assertIsNotEnabled()
        composeRule.onNodeWithText("Approve automatically").assertIsNotEnabled()
        composeRule.onNodeWithText("Full access").assertIsNotEnabled()
        composeRule.onNodeWithText("Refresh").assertIsEnabled()
    }

    @Test
    fun disconnectedStateExplainsConnectionAndDisablesModes() {
        setPermissionContent(
            permissionMode = SessionPermissionMode.ASK,
            connected = false,
        )

        composeRule
            .onNodeWithText("Connect to the desktop to change the permission mode.")
            .assertIsDisplayed()
        composeRule.onNodeWithText("Ask first").assertIsNotEnabled()
        composeRule.onNodeWithText("Approve automatically").assertIsNotEnabled()
        composeRule.onNodeWithText("Full access").assertIsNotEnabled()
    }

    private fun setPermissionContent(
        permissionMode: SessionPermissionMode?,
        connected: Boolean = true,
        onIntent: (RemoteSessionIntent) -> Unit = {},
    ) {
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                PermissionSection(
                    state = readyState(permissionMode),
                    connected = connected,
                    onIntent = onIntent,
                    modifier = Modifier,
                )
            }
        }
    }

    private fun readyState(permissionMode: SessionPermissionMode?) = RemoteSessionUiState.Ready(
        sessions = emptyList(),
        selectedSessionId = null,
        timeline = null,
        busy = false,
        permissionMode = permissionMode,
        permissionModeFailure = null,
        query = "",
        agentFilter = SessionAgentFilter.ALL,
        hasMore = false,
        hasMoreMessages = false,
        modelCatalog = null,
    )
}
