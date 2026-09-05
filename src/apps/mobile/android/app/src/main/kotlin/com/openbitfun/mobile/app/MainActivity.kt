package com.openbitfun.mobile.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.openbitfun.mobile.app.ui.shell.MobileScreen
import com.openbitfun.mobile.app.platform.AppLocaleController
import com.openbitfun.mobile.app.ui.preview.MobileDesignGallery
import com.openbitfun.mobile.app.ui.preview.mobileDesignScenario
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import com.openbitfun.mobile.app.viewmodel.AppSettingsViewModel
import com.openbitfun.mobile.app.viewmodel.AppThemeMode

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        AppLocaleController.applySaved(this)
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            if (intent.getBooleanExtra(DESIGN_PREVIEW_EXTRA, false)) {
                val scenario = mobileDesignScenario(intent.getStringExtra(DESIGN_SCENARIO_EXTRA))
                MobileDesignGallery(scenario = scenario, dark = scenario.appearance == "dark")
                return@setContent
            }
            val settings: AppSettingsViewModel = viewModel(factory = AppSettingsViewModel.Factory)
            val theme by settings.theme.collectAsStateWithLifecycle()
            val dark = when (theme) {
                AppThemeMode.SYSTEM -> isSystemInDarkTheme()
                AppThemeMode.LIGHT -> false
                AppThemeMode.DARK -> true
            }
            OpenBitFunTheme(dark = dark) {
                MobileScreen()
            }
        }
    }

    private companion object {
        const val DESIGN_PREVIEW_EXTRA = "openbitfun.design_preview"
        const val DESIGN_SCENARIO_EXTRA = "openbitfun.design_scenario"
    }
}
