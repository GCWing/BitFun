package com.openbitfun.mobile.app

import androidx.compose.foundation.layout.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.unit.dp
import com.openbitfun.mobile.app.ui.common.connectionSheetTextStyle
import com.openbitfun.mobile.app.ui.common.ConnectionSheetHeader
import com.openbitfun.mobile.app.ui.common.ConnectionSheetFooter
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class ConnectionSheetChromeTest {
    @get:Rule val composeRule = createComposeRule()

    @Test fun closeAndManualActionsKeepHarmonyGeometryAndRemainClickable() {
        var closed = false
        var manual = false
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                Column(Modifier.requiredSize(360.dp, 700.dp)) {
                    ConnectionSheetHeader { closed = true }
                    Spacer(Modifier.weight(1f))
                    ConnectionSheetFooter("Pair manually", onClick = { manual = true })
                }
            }
        }
        composeRule.onNodeWithContentDescription(
            androidx.test.platform.app.InstrumentationRegistry.getInstrumentation().targetContext.getString(R.string.common_close))
            .assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp).performClick()
        composeRule.onNodeWithText("Pair manually").assertHeightIsEqualTo(48.dp)
            .assertWidthIsEqualTo(320.dp).performClick()
        composeRule.runOnIdle { assertTrue(closed); assertTrue(manual) }
    }
    @Test fun chineseSheetTextUsesTheReferenceLineBoxes() {
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                Column {
                    androidx.compose.material3.Text("扫描桌面端二维码",
                        style = androidx.compose.material3.MaterialTheme.typography.displayMedium.connectionSheetTextStyle())
                    androidx.compose.material3.Text("第一行说明\n第二行说明",
                        style = androidx.compose.material3.MaterialTheme.typography.bodyMedium.connectionSheetTextStyle())
                }
            }
        }
        composeRule.onNodeWithText("扫描桌面端二维码").assertIsDisplayed().assertHeightIsEqualTo(28.dp)
        val body = composeRule.onNodeWithText("第一行说明\n第二行说明").assertIsDisplayed()
        // Android rounds the first/last font baselines independently; preserve
        // the two 21dp lines within one layout unit without padding the glyphs.
        val bounds = body.getUnclippedBoundsInRoot()
        assertTrue(kotlin.math.abs((bounds.bottom - bounds.top).value - 42f) <= 1f)
    }

    @Test fun compactLoginKeepsContentGeometryAtPhoneAndWideWidths() {
        var width by androidx.compose.runtime.mutableStateOf(386.dp)
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                com.openbitfun.mobile.app.ui.account.AccountLoginPage(
                    state = com.openbitfun.mobile.core.feature.account.AccountUiState.SignedOut,
                    onBack = {}, onLogin = {},
                    modifier = Modifier.requiredWidth(width).testTag("login-panel"))
            }
        }
        val context = androidx.test.platform.app.InstrumentationRegistry.getInstrumentation().targetContext
        for (panelWidth in listOf(386.dp, 560.dp)) {
            composeRule.runOnIdle { width = panelWidth }
            val panel = composeRule.onNodeWithTag("login-panel").assertHeightIsEqualTo(280.dp)
                .assertWidthIsEqualTo(panelWidth).getUnclippedBoundsInRoot()
            val labels = composeRule.onAllNodesWithText(context.getString(R.string.account_login_title))
            val title = labels[0].getUnclippedBoundsInRoot()
            val action = labels[1].getUnclippedBoundsInRoot()
            assertTrue(kotlin.math.abs((title.top - panel.top).value - 56f) < 1f)
            assertTrue(kotlin.math.abs((action.top - panel.top).value - 208f) < 1f)
            labels[1].assertHeightIsEqualTo(48.dp).assertWidthIsEqualTo(panelWidth - 40.dp)
        }
    }

}
