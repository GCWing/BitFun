package com.openbitfun.mobile.app

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import com.openbitfun.mobile.app.ui.chat.COMPOSER_INPUT_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.ComposerBar
import com.openbitfun.mobile.app.ui.chat.MODEL_CONTROL_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.MODEL_SELECTOR_OPTION_TEST_TAG_PREFIX
import com.openbitfun.mobile.app.ui.chat.MODEL_SELECTOR_TEST_TAG
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import com.openbitfun.mobile.core.feature.connection.ConnectionPhase
import com.openbitfun.mobile.core.feature.session.ChatComposerCapabilities
import com.openbitfun.mobile.core.feature.session.ModelOption
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

class ComposerModelSelectorTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun theExpandedComposerSelectsAConfiguredModel() {
        var selected = "account-primary"
        val models = listOf(
            ModelOption("account-primary", "Primary", "Account model", true),
            ModelOption("account-fast", "Fast", "Account model", false),
        )
        composeRule.setContent {
            OpenBitFunTheme(dark = false) {
                ComposerBar(
                    draft = "",
                    images = emptyList(),
                    busy = false,
                    streaming = false,
                    phase = ConnectionPhase.DISCONNECTED,
                    model = models.first(),
                    modelOptions = models,
                    capabilities = ChatComposerCapabilities.GeneralChat,
                    placeholder = "Message",
                    onDraftChange = {},
                    onRemoveImage = {},
                    onAttach = {},
                    onVoice = {},
                    onSend = {},
                    onStop = {},
                    onOpenModels = {},
                    onSelectModel = { selected = it },
                    modifier = androidx.compose.ui.Modifier,
                )
            }
        }

        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG).performClick()
        composeRule.onNodeWithTag(MODEL_CONTROL_TEST_TAG).assertIsDisplayed().performClick()
        composeRule.onNodeWithTag(MODEL_SELECTOR_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(MODEL_SELECTOR_OPTION_TEST_TAG_PREFIX + "account-fast")
            .performClick()

        assertEquals("account-fast", selected)
    }
}
