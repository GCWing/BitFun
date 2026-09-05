package com.openbitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.openbitfun.mobile.app.ui.remote.CONNECT_ACCOUNT_DEVICE_REFRESH_TEST_TAG
import com.openbitfun.mobile.app.ui.remote.CONNECT_ACCOUNT_DEVICE_ROW_TEST_TAG_PREFIX
import com.openbitfun.mobile.app.ui.remote.CONNECT_ACCOUNT_DEVICE_SCAN_TEST_TAG
import com.openbitfun.mobile.app.ui.remote.ConnectAccountDeviceScreen
import com.openbitfun.mobile.app.ui.remote.AccountRemoteScreen
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import com.openbitfun.mobile.core.feature.account.AccountDeviceUi
import com.openbitfun.mobile.core.feature.account.AccountUiState
import com.openbitfun.mobile.core.feature.connection.ConnectionPhase
import com.openbitfun.mobile.core.feature.layout.SettingsPlacement
import com.openbitfun.mobile.core.feature.layout.SettingsPlacementMode
import com.openbitfun.mobile.core.feature.session.RemoteSessionUiState
import com.openbitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import org.junit.Rule
import org.junit.Test
import org.junit.Assert.assertEquals

class AccountRemoteScreenTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun aSelectedAccountDeviceBypassesThePairingForm() {
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                AccountRemoteScreen(
                    remoteState = RemoteSessionUiState.Idle,
                    workspaceState = RemoteWorkspaceUiState.Idle,
                    deviceId = "device-1",
                    deviceName = "Studio Mac",
                    createDevices = emptyList(),
                    accountUsername = "tester",
                    phase = ConnectionPhase.CONNECTED,
                    settingsPlacement = SettingsPlacement(
                        mode = SettingsPlacementMode.BOTTOM,
                        width = 0,
                        height = 0,
                        maxHeight = 0,
                    ),
                    sessionDetailsPlacement = SettingsPlacement(
                        mode = SettingsPlacementMode.BOTTOM,
                        width = 0,
                        height = 0,
                        maxHeight = 0,
                    ),
                    viewSettingsPlacement = SettingsPlacement(
                        mode = SettingsPlacementMode.BOTTOM,
                        width = 0,
                        height = 0,
                        maxHeight = 0,
                    ),
                    onOpenRemoteSettings = {},
                    onCreateDevicePick = {},
                    onSessionIntent = {},
                    onWorkspaceIntent = {},
                    modifier = Modifier,
                )
            }
        }

        composeRule.onAllNodesWithText("Connect to a desktop").assertCountEquals(0)
    }

    @Test
    fun aSignedInAccountWithoutATargetCanRefreshSelectOrScan() {
        var refreshes = 0
        var selected = ""
        var scans = 0
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                ConnectAccountDeviceScreen(
                    state = AccountUiState.Ready(
                        userId = "user-1",
                        username = "tester",
                        devices = listOf(
                            AccountDeviceUi("desk-1", "Studio Mac", online = true, lastSeenAt = null),
                            AccountDeviceUi("desk-2", "Office PC", online = false, lastSeenAt = null),
                        ),
                        selectedDeviceId = null,
                        selectedDeviceName = null,
                    ),
                    onBack = {},
                    onRefresh = { refreshes += 1 },
                    onSelect = { selected = it },
                    onOpenScanner = { scans += 1 },
                    modifier = Modifier,
                )
            }
        }

        composeRule.onNodeWithText("Choose a desktop").assertIsDisplayed()
        composeRule.onNodeWithTag(CONNECT_ACCOUNT_DEVICE_REFRESH_TEST_TAG).performClick()
        composeRule.onNodeWithTag(CONNECT_ACCOUNT_DEVICE_ROW_TEST_TAG_PREFIX + "desk-1").performClick()
        composeRule.onNodeWithTag(CONNECT_ACCOUNT_DEVICE_ROW_TEST_TAG_PREFIX + "desk-2").performClick()
        composeRule.onNodeWithTag(CONNECT_ACCOUNT_DEVICE_SCAN_TEST_TAG).performClick()

        assertEquals(1, refreshes)
        assertEquals("desk-1", selected)
        assertEquals(1, scans)
    }

    @Test
    fun aSelectedOfflineAccountDeviceCanReconnect() {
        var selected = ""
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                ConnectAccountDeviceScreen(
                    state = AccountUiState.Ready(
                        userId = "user-1",
                        username = "tester",
                        devices = listOf(
                            AccountDeviceUi("desk-1", "Studio Mac", online = true, lastSeenAt = null),
                            AccountDeviceUi("desk-2", "Office PC", online = false, lastSeenAt = null),
                        ),
                        selectedDeviceId = "desk-2",
                        selectedDeviceName = "Office PC",
                    ),
                    onBack = {},
                    onRefresh = {},
                    onSelect = { selected = it },
                    onOpenScanner = {},
                    modifier = Modifier,
                )
            }
        }

        composeRule.onNodeWithTag(CONNECT_ACCOUNT_DEVICE_ROW_TEST_TAG_PREFIX + "desk-2").performClick()

        assertEquals("desk-2", selected)
    }
}
