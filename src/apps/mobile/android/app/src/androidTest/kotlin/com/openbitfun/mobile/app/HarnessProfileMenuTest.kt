package com.openbitfun.mobile.app

import android.graphics.Bitmap
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import androidx.test.platform.app.InstrumentationRegistry
import com.openbitfun.mobile.app.ui.remote.ProjectCreateControl
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import java.io.File

class HarnessProfileMenuTest {
    @get:Rule val composeRule = createComposeRule()
    @Test fun supportedHostOffersAllProfilesAndSendsCanonicalAgent() = verify(true, false)
    @Test fun legacyHostRetainsCodeAndCoworkInDarkMode() = verify(false, true)

    private fun verify(supported: Boolean, dark: Boolean) {
        var selected: String? = null
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        composeRule.setContent {
            OpenBitFunTheme(dark = dark) {
                Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).padding(32.dp)) {
                    var expanded by remember { mutableStateOf(true) }
                    ProjectCreateControl(
                        supportsHarnessProfiles = supported, path = "/remote/project", expanded = expanded,
                        onToggle = { expanded = !expanded }, onDismiss = { expanded = false },
                        onCreateAgent = { selected = it; expanded = false },
                    )
                }
            }
        }
        val label = context.getString(if (supported) R.string.harness_ultimate else R.string.sessions_filter_code)
        composeRule.onNodeWithText(label).assertIsDisplayed()
        if (supported) {
            composeRule.onNodeWithText(context.getString(R.string.harness_minimal)).assertIsDisplayed()
            composeRule.onNodeWithText(context.getString(R.string.harness_standard)).assertIsDisplayed()
        }
        composeRule.waitForIdle()
        val image = InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot()
        File(context.getExternalFilesDir(null), "harness-${if (supported) "light" else "legacy-dark"}.png").outputStream().use {
            image.compress(Bitmap.CompressFormat.PNG, 100, it)
        }
        composeRule.onNodeWithText(label).performClick()
        composeRule.runOnIdle { assertEquals(if (supported) "Ultra" else "code", selected) }
    }
}
