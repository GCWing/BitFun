package com.openbitfun.mobile.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.openbitfun.mobile.core.feature.CloudSettingsSource
import com.openbitfun.mobile.core.feature.generalchat.GeneralChatIntent
import com.openbitfun.mobile.core.feature.generalchat.GeneralChatStore
import com.openbitfun.mobile.core.feature.generalchat.GeneralChatUiState
import com.openbitfun.mobile.core.feature.generalchat.create
import kotlinx.coroutines.flow.StateFlow

internal class GeneralChatViewModel(application: Application) : AndroidViewModel(application) {
    private val store = GeneralChatStore.create(viewModelScope, application)
    val state: StateFlow<GeneralChatUiState> = store.state

    fun dispatch(intent: GeneralChatIntent) {
        store.dispatch(intent)
    }

    /**
     * Point the model catalog at the signed-in account, or at nothing.
     *
     * Called on every sign-in change rather than once, because the source is
     * bound to one session: passing null on sign-out is what drops the account's
     * models, and passing a fresh one on sign-in is what loads the next user's.
     */
    fun bindCloudSettings(source: CloudSettingsSource?) {
        store.bindCloudSettings(source)
    }

    override fun onCleared() {
        store.stop()
        super.onCleared()
    }

    companion object {
        val Factory: ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(
                modelClass: Class<T>,
                extras: androidx.lifecycle.viewmodel.CreationExtras,
            ): T = GeneralChatViewModel(
                extras[ViewModelProvider.AndroidViewModelFactory.APPLICATION_KEY]!!,
            ) as T
        }
    }
}
