package com.openbitfun.mobile.app

import android.content.Context
import android.content.ContextWrapper
import android.content.SharedPreferences
import android.content.pm.PackageManager
import androidx.activity.compose.LocalActivityResultRegistryOwner
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import com.openbitfun.mobile.app.platform.NotificationOnboarding as OnboardingStore
import com.openbitfun.mobile.app.ui.shell.NotificationOnboarding
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class NotificationOnboardingTest {
    @get:Rule val composeRule = createComposeRule()

    @Test fun laterPersistsAndDoesNotRequestSystemPermission() {
        val base = InstrumentationRegistry.getInstrumentation().targetContext
        val name = "notification_onboarding_test_${System.nanoTime()}"
        val context = object : ContextWrapper(base) {
            override fun getSharedPreferences(ignored: String, mode: Int): SharedPreferences =
                base.getSharedPreferences(name, mode)
            override fun checkSelfPermission(permission: String) = PackageManager.PERMISSION_DENIED
        }
        try {
            // Missing keys from an older install remain readable and offer onboarding.
            assertTrue(OnboardingStore(context).shouldOffer())
            composeRule.setContent {
                val registryOwner = requireNotNull(LocalActivityResultRegistryOwner.current)
                CompositionLocalProvider(
                    LocalContext provides context,
                    LocalActivityResultRegistryOwner provides registryOwner,
                ) {
                    OpenBitFunTheme(dark = false) { NotificationOnboarding() }
                }
            }
            composeRule.onNodeWithText(context.getString(R.string.notification_onboarding_title)).assertIsDisplayed()
            composeRule.onNodeWithText(context.getString(R.string.notification_onboarding_later)).performClick()
            composeRule.waitForIdle()
            assertFalse(OnboardingStore(context).shouldOffer())
            composeRule.onNodeWithText(context.getString(R.string.notification_onboarding_title)).assertDoesNotExist()
        } finally {
            base.deleteSharedPreferences(name)
        }
    }

    @Test fun existingGrantSkipsIntroductionWithoutResettingStoredData() {
        val base = InstrumentationRegistry.getInstrumentation().targetContext
        val name = "notification_onboarding_test_${System.nanoTime()}"
        val context = object : ContextWrapper(base) {
            override fun getSharedPreferences(ignored: String, mode: Int): SharedPreferences =
                base.getSharedPreferences(name, mode)
            override fun checkSelfPermission(permission: String) = PackageManager.PERMISSION_GRANTED
        }
        try {
            context.getSharedPreferences(name, 0).edit().putString("future_field", "retained").commit()
            assertFalse(OnboardingStore(context).shouldOffer())
            assertTrue(context.getSharedPreferences(name, 0).contains("future_field"))
        } finally {
            base.deleteSharedPreferences(name)
        }
    }
}
