package com.bitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.bitfun.mobile.app.ui.settings.MODEL_CATALOG_FAILURE_TEST_TAG
import com.bitfun.mobile.app.ui.settings.MODEL_CATALOG_RETRY_TEST_TAG
import com.bitfun.mobile.app.ui.settings.RemoteSettingsSheet
import com.bitfun.mobile.app.ui.theme.BitFunTheme
import com.bitfun.mobile.core.feature.session.ModelCatalogFailure
import com.bitfun.mobile.core.feature.session.RemoteSessionIntent
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.session.SessionAgentFilter
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class RemoteSettingsSheetTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun transientCatalogFailureShowsAnErrorAndRetryDispatchesTheIntent() {
        val intents = mutableListOf<RemoteSessionIntent>()
        setSheetContent(
            state = readyState(modelCatalogFailure = ModelCatalogFailure.LOAD_FAILED),
            onIntent = { intents += it },
        )

        composeRule
            .onNodeWithTag(MODEL_CATALOG_FAILURE_TEST_TAG)
            .assertIsDisplayed()
        composeRule
            .onNodeWithText("Could not load the available models. Try again.")
            .assertIsDisplayed()
        composeRule
            .onNodeWithTag(MODEL_CATALOG_RETRY_TEST_TAG)
            .assertIsDisplayed()
            .performClick()

        assertEquals(
            listOf<RemoteSessionIntent>(RemoteSessionIntent.RefreshModelCatalog),
            intents,
        )
    }

    @Test
    fun unsupportedByPeerIsExplicitAndOffersNoRetry() {
        val intents = mutableListOf<RemoteSessionIntent>()
        setSheetContent(
            state = readyState(modelCatalogFailure = ModelCatalogFailure.UNSUPPORTED_BY_PEER),
            onIntent = { intents += it },
        )

        composeRule
            .onNodeWithText(
                "The connected desktop does not support choosing models from this app.",
            )
            .assertIsDisplayed()
        composeRule.onNodeWithTag(MODEL_CATALOG_RETRY_TEST_TAG).assertDoesNotExist()
        assertTrue(intents.isEmpty())
    }

    private fun setSheetContent(
        state: RemoteSessionUiState.Ready,
        onIntent: (RemoteSessionIntent) -> Unit = {},
    ) {
        composeRule.setContent {
            BitFunTheme(dark = false) {
                RemoteSettingsSheet(
                    state = state,
                    sessionId = "s-code",
                    onIntent = onIntent,
                    modifier = Modifier,
                )
            }
        }
    }

    private fun readyState(modelCatalogFailure: ModelCatalogFailure?) = RemoteSessionUiState.Ready(
        sessions = emptyList(),
        selectedSessionId = "s-code",
        timeline = null,
        busy = false,
        permissionMode = null,
        permissionModeFailure = null,
        query = "",
        agentFilter = SessionAgentFilter.ALL,
        hasMore = false,
        hasMoreMessages = false,
        modelCatalog = null,
        modelCatalogFailure = modelCatalogFailure,
        draft = "",
    )
}
