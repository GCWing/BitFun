package com.openbitfun.mobile.app

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.openbitfun.mobile.app.ui.chat.tool.ToolConfirmationPanel
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import org.junit.Rule
import org.junit.Test

class ToolConfirmationPanelTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun approvalShowsVerdictButtonsWithoutEditableInput() {
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                ToolConfirmationPanel(
                    canApprove = true,
                    canReject = true,
                    enabled = true,
                    onApprove = {},
                    onReject = {},
                )
            }
        }

        composeRule.onNodeWithText("Approve").assertIsDisplayed()
        composeRule.onNodeWithText("Reject").assertIsDisplayed()
        composeRule.onAllNodes(hasSetTextAction()).assertCountEquals(0)
    }
}
