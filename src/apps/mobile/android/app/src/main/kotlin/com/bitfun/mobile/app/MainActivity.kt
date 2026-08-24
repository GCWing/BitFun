package com.bitfun.mobile.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.bitfun.mobile.app.ui.shell.MobileScreen
import com.bitfun.mobile.app.platform.AppLocaleController
import com.bitfun.mobile.app.ui.preview.MobileDesignGallery
import com.bitfun.mobile.app.ui.preview.mobileDesignScenario
import com.bitfun.mobile.app.ui.theme.BitFunTheme
import com.bitfun.mobile.app.viewmodel.AppSettingsViewModel
import com.bitfun.mobile.app.viewmodel.AppThemeMode

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
            BitFunTheme(dark = dark) {
                MobileScreen()
            }
        }
    }

    private companion object {
        const val DESIGN_PREVIEW_EXTRA = "bitfun.design_preview"
        const val DESIGN_SCENARIO_EXTRA = "bitfun.design_scenario"
    }
}
