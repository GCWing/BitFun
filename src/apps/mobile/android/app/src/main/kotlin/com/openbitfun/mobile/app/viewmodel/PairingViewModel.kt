package com.openbitfun.mobile.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.openbitfun.mobile.app.platform.LogcatCoreLog
import com.openbitfun.mobile.app.platform.deviceIdentity
import com.openbitfun.mobile.core.feature.pairing.PairingIntent
import com.openbitfun.mobile.core.feature.pairing.PairingStore
import com.openbitfun.mobile.core.feature.pairing.PairingUiState
import com.openbitfun.mobile.core.feature.pairing.create
import com.openbitfun.mobile.core.feature.session.RemoteSessionIntent
import com.openbitfun.mobile.core.feature.session.RemoteSessionStore
import com.openbitfun.mobile.core.feature.session.RemoteSessionUiState
import com.openbitfun.mobile.core.feature.workspace.RemoteWorkspaceIntent
import com.openbitfun.mobile.core.feature.workspace.RemoteWorkspaceStore
import com.openbitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch

/**
 * Owns the store for as long as the screen exists.
 *
 * The store is scoped to [viewModelScope], so a rotation mid-handshake does not
 * restart it and leaving the screen cancels it.
 */
internal class PairingViewModel(application: Application) : AndroidViewModel(application) {
    private val store = PairingStore.create(
        scope = viewModelScope,
        // The credential cooldown outlives this view model — a counter that a
        // rotation resets would not be one — so the store is handed the
        // application rather than kept in memory here.
        context = application,
        device = application.deviceIdentity(),
        log = LogcatCoreLog,
    )

    val state: StateFlow<PairingUiState> = store.state
    private val _remoteState = MutableStateFlow<RemoteSessionUiState>(RemoteSessionUiState.Idle)
    val remoteState: StateFlow<RemoteSessionUiState> = _remoteState.asStateFlow()
    private var remoteStore: RemoteSessionStore? = null
    private var remoteStateJob: Job? = null
    private val _workspaceState = MutableStateFlow<RemoteWorkspaceUiState>(RemoteWorkspaceUiState.Idle)
    val workspaceState: StateFlow<RemoteWorkspaceUiState> = _workspaceState.asStateFlow()
    private var workspaceStore: RemoteWorkspaceStore? = null
    private var workspaceStateJob: Job? = null

    init {
        viewModelScope.launch {
            store.state.collect { current ->
                if (current is PairingUiState.Paired && remoteStore == null) {
                    val created = store.createSessionStore(viewModelScope) ?: return@collect
                    remoteStore = created
                    remoteStateJob = launch {
                        created.state.collect { _remoteState.value = it }
                    }
                    created.dispatch(RemoteSessionIntent.Load)
                    val workspace = store.createWorkspaceStore(viewModelScope)
                    workspaceStore = workspace
                    workspaceStateJob = workspace?.let { workspaceStore ->
                        launch {
                            workspaceStore.state.collect { _workspaceState.value = it }
                        }
                    }
                    workspace?.dispatch(RemoteWorkspaceIntent.Load)
                } else if (current !is PairingUiState.Paired && remoteStore != null) {
                    remoteStateJob?.cancel()
                    remoteStateJob = null
                    remoteStore?.dispatch(RemoteSessionIntent.Stop)
                    remoteStore = null
                    _remoteState.value = RemoteSessionUiState.Idle
                    workspaceStateJob?.cancel()
                    workspaceStateJob = null
                    workspaceStore?.dispatch(RemoteWorkspaceIntent.Stop)
                    workspaceStore = null
                    _workspaceState.value = RemoteWorkspaceUiState.Idle
                }
            }
        }
    }

    fun dispatch(intent: PairingIntent) {
        store.dispatch(intent)
    }

    fun dispatchSession(intent: RemoteSessionIntent) {
        remoteStore?.dispatch(intent)
    }

    fun dispatchWorkspace(intent: RemoteWorkspaceIntent) {
        workspaceStore?.dispatch(intent)
    }

    companion object {
        val Factory: ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(
                modelClass: Class<T>,
                extras: androidx.lifecycle.viewmodel.CreationExtras,
            ): T = PairingViewModel(
                extras[ViewModelProvider.AndroidViewModelFactory.APPLICATION_KEY]!!,
            ) as T
        }
    }
}
