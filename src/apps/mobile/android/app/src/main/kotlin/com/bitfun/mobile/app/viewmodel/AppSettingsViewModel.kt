package com.bitfun.mobile.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

internal enum class AppThemeMode {
    SYSTEM,
    LIGHT,
    DARK,
}

internal class AppSettingsViewModel(application: Application) : AndroidViewModel(application) {
    private val preferences = application.getSharedPreferences("bitfun_app_settings", Application.MODE_PRIVATE)
    private val _theme = MutableStateFlow(
        runCatching { AppThemeMode.valueOf(preferences.getString("theme", null).orEmpty()) }
            .getOrDefault(AppThemeMode.SYSTEM),
    )
    val theme: StateFlow<AppThemeMode> = _theme.asStateFlow()

    fun setTheme(theme: AppThemeMode) {
        preferences.edit().putString("theme", theme.name).apply()
        _theme.value = theme
    }

    companion object {
        val Factory: ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(
                modelClass: Class<T>,
                extras: androidx.lifecycle.viewmodel.CreationExtras,
            ): T = AppSettingsViewModel(
                extras[ViewModelProvider.AndroidViewModelFactory.APPLICATION_KEY]!!,
            ) as T
        }
    }
}
