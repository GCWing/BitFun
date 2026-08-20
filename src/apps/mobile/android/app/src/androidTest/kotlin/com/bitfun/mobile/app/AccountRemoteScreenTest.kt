package com.bitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import com.bitfun.mobile.app.ui.remote.AccountRemoteScreen
import com.bitfun.mobile.app.ui.theme.BitFunTheme
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import org.junit.Rule
import org.junit.Test

class AccountRemoteScreenTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun aSelectedAccountDeviceBypassesThePairingForm() {
        composeRule.setContent {
            BitFunTheme(dark = false) {
                AccountRemoteScreen(
                    remoteState = RemoteSessionUiState.Idle,
                    workspaceState = RemoteWorkspaceUiState.Idle,
                    deviceId = "device-1",
                    deviceName = "Studio Mac",
                    accountUsername = "tester",
                    phase = ConnectionPhase.CONNECTED,
                    onSessionIntent = {},
                    onWorkspaceIntent = {},
                    modifier = Modifier,
                )
            }
        }

        composeRule.onAllNodesWithText("Connect to a desktop").assertCountEquals(0)
    }
}
