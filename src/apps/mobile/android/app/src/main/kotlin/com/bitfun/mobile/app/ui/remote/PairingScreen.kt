package com.bitfun.mobile.app.ui.remote

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.chat.ConversationView
import com.bitfun.mobile.app.viewmodel.PairingViewModel
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.connection.connectionPhase
import com.bitfun.mobile.core.feature.pairing.ConnectionLiveness
import com.bitfun.mobile.core.feature.pairing.PairedWorkspace
import com.bitfun.mobile.core.feature.pairing.PairingIntent
import com.bitfun.mobile.core.feature.pairing.PairingUiState
import com.bitfun.mobile.core.feature.session.ConversationHeaderPresenter
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceIntent
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState

/**
 * The remote surface: pair, then either the session list or one open session.
 *
 * Everything below the seam is a [PairingUiState]; this file decides layout and
 * wording and nothing else. The list and the conversation replace each other
 * rather than stacking, matching `pages/RemoteSurfaceHost.ets` — a transcript
 * needs the whole height and its own scroll.
 */
@Composable
internal fun PairingScreen(
    modifier: Modifier,
    viewModel: PairingViewModel = viewModel(factory = PairingViewModel.Factory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val remoteState by viewModel.remoteState.collectAsStateWithLifecycle()
    val workspaceState by viewModel.workspaceState.collectAsStateWithLifecycle()
    // The heartbeat runs only while this surface is both composed and resumed:
    // a ping every fifteen seconds from a backgrounded app buys nothing and
    // costs a wake-up, and coming back is exactly when the answer is stale.
    LifecycleResumeEffect(viewModel) {
        viewModel.dispatch(PairingIntent.Foreground)
        onPauseOrDispose { viewModel.dispatch(PairingIntent.Background) }
    }

    when (val current = state) {
        is PairingUiState.Paired -> {
            RemoteConnectedScreen(
                remoteState = remoteState,
                workspaceState = workspaceState,
                phase = current.connectionPhase(),
                deviceId = current.workspace.roomLabel,
                desktopName = "",
                onSessionIntent = viewModel::dispatchSession,
                onWorkspaceIntent = viewModel::dispatchWorkspace,
                connectionDetails = {
                    PairedDetails(
                        workspace = current.workspace,
                        liveness = current.liveness,
                        onVerify = { viewModel.dispatch(PairingIntent.Verify) },
                        onDisconnect = { viewModel.dispatch(PairingIntent.Disconnect) },
                    )
                },
                modifier = modifier,
            )
        }

        else -> ConnectView(
            state = current,
            onSubmit = viewModel::dispatch,
            onDismiss = { viewModel.dispatch(PairingIntent.Dismiss) },
            modifier = modifier,
        )
    }
}

/** The account-device route, which bypasses the QR pairing form entirely. */
@Composable
internal fun AccountRemoteScreen(
    remoteState: RemoteSessionUiState,
    workspaceState: RemoteWorkspaceUiState,
    deviceId: String,
    deviceName: String,
    accountUsername: String,
    onSessionIntent: (com.bitfun.mobile.core.feature.session.RemoteSessionIntent) -> Unit,
    onWorkspaceIntent: (RemoteWorkspaceIntent) -> Unit,
    modifier: Modifier,
) {
    RemoteConnectedScreen(
        remoteState = remoteState,
        workspaceState = workspaceState,
        phase = ConnectionPhase.CONNECTED,
        deviceId = deviceId,
        desktopName = deviceName,
        onSessionIntent = onSessionIntent,
        onWorkspaceIntent = onWorkspaceIntent,
        connectionDetails = {
            AccountDeviceDetails(deviceName = deviceName, accountUsername = accountUsername)
        },
        modifier = modifier,
    )
}

@Composable
private fun RemoteConnectedScreen(
    remoteState: RemoteSessionUiState,
    workspaceState: RemoteWorkspaceUiState,
    phase: ConnectionPhase,
    deviceId: String,
    desktopName: String,
    onSessionIntent: (com.bitfun.mobile.core.feature.session.RemoteSessionIntent) -> Unit,
    onWorkspaceIntent: (RemoteWorkspaceIntent) -> Unit,
    connectionDetails: @Composable () -> Unit,
    modifier: Modifier,
) {
    // Which session the user asked to open. Not the same as the store's
    // `selectedSessionId`, which stays set after the user comes back to the list.
    var openSessionId by rememberSaveable(deviceId) { mutableStateOf<String?>(null) }
    var creating by rememberSaveable(deviceId) { mutableStateOf(false) }
    val conversation = (remoteState as? RemoteSessionUiState.Ready)?.takeIf {
        openSessionId != null && it.selectedSessionId == openSessionId && it.timeline != null
    }
    if (conversation != null) {
        ConversationView(
            state = conversation,
            phase = phase,
            onBack = { openSessionId = null },
            onIntent = onSessionIntent,
            contextTitle = ConversationHeaderPresenter.contextTitle(
                desktopName = desktopName,
                workspaceBranch = (workspaceState as? RemoteWorkspaceUiState.Ready)
                    ?.selected?.gitBranch.orEmpty(),
            ),
            onOpenFile = { path, label ->
                onWorkspaceIntent(
                    RemoteWorkspaceIntent.OpenFile(
                        path,
                        label,
                        conversation.selectedSessionId.orEmpty(),
                    ),
                )
            },
            previewingRemotePath = workspaceState.previewingRemotePath(),
            previewLoading = workspaceState.previewLoading(),
            modifier = modifier,
        )
    } else if (creating) {
        CreateSessionRoute(
            sessionState = remoteState,
            workspaceState = workspaceState,
            phase = phase,
            deviceId = deviceId,
            onBack = { creating = false },
            onCreated = {
                creating = false
                openSessionId = it
            },
            onWorkspaceIntent = onWorkspaceIntent,
            onIntent = onSessionIntent,
            modifier = modifier,
        )
    } else {
        RemoteSessionListView(
            state = remoteState,
            workspaceState = workspaceState,
            connectionDetails = connectionDetails,
            onIntent = onSessionIntent,
            onWorkspaceIntent = onWorkspaceIntent,
            onOpen = { openSessionId = it },
            onCreate = { creating = true },
            modifier = modifier,
        )
    }
}

@Composable
private fun AccountDeviceDetails(deviceName: String, accountUsername: String) {
    Text(stringResource(R.string.paired_title), style = MaterialTheme.typography.headlineSmall)
    Text(
        stringResource(R.string.account_device_controlling, deviceName),
        style = MaterialTheme.typography.bodyMedium,
    )
    if (accountUsername.isNotBlank()) {
        Text(
            stringResource(R.string.paired_user, accountUsername),
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

@Composable
internal fun RemoteWorkspacePanel(
    state: RemoteWorkspaceUiState,
    sessionId: String,
    onIntent: (RemoteWorkspaceIntent) -> Unit,
    // False wherever the shell places the preview itself: the file gets a pane
    // or the whole page there, and a second copy inline under the list would be
    // the same document twice.
    showPreview: Boolean = true,
) {
    var fileReference by rememberSaveable { mutableStateOf("") }
    Text(stringResource(R.string.workspace_title), style = MaterialTheme.typography.titleLarge)
    when (state) {
        RemoteWorkspaceUiState.Idle -> Unit
        RemoteWorkspaceUiState.Loading -> CircularProgressIndicator()
        is RemoteWorkspaceUiState.Failed -> {
            Text(stringResource(R.string.workspace_failed), color = MaterialTheme.colorScheme.error)
            TextButton(onClick = { onIntent(RemoteWorkspaceIntent.Load) }) {
                Text(stringResource(R.string.sessions_refresh))
            }
        }
        is RemoteWorkspaceUiState.Ready -> {
            state.selected?.let { selected ->
                Text(selected.name, style = MaterialTheme.typography.titleMedium)
                if (selected.gitBranch.isNotEmpty()) Text(selected.gitBranch)
            }
            state.workspaces.forEach { workspace ->
                TextButton(
                    onClick = { onIntent(RemoteWorkspaceIntent.SelectWorkspace(workspace.path)) },
                    enabled = !state.busy && state.selected?.path != workspace.path,
                ) { Text(workspace.name) }
            }
            if (state.assistants.isNotEmpty()) {
                Text(stringResource(R.string.assistants_title), style = MaterialTheme.typography.titleSmall)
                state.assistants.forEach { assistant ->
                    TextButton(
                        onClick = { onIntent(RemoteWorkspaceIntent.SelectAssistant(assistant.path)) },
                        enabled = !state.busy,
                    ) { Text(assistant.name) }
                }
            }
            OutlinedTextField(
                value = fileReference,
                onValueChange = { fileReference = it },
                label = { Text(stringResource(R.string.file_reference_label)) },
                enabled = !state.busy,
                modifier = Modifier.fillMaxWidth(),
            )
            Button(
                onClick = {
                    onIntent(RemoteWorkspaceIntent.OpenFile(fileReference, "", sessionId))
                },
                enabled = fileReference.isNotBlank(),
            ) { Text(stringResource(R.string.file_preview_open)) }
            if (showPreview) {
                FilePreviewSurface(
                    preview = state.preview,
                    onIntent = onIntent,
                    modifier = Modifier,
                )
            }
        }
    }
}


internal const val CONNECTION_RETRY_TEST_TAG: String = "connection-retry"

@Composable
internal fun PairedDetails(
    workspace: PairedWorkspace,
    liveness: ConnectionLiveness,
    onVerify: () -> Unit,
    onDisconnect: () -> Unit,
) {
    Text(stringResource(R.string.paired_title), style = MaterialTheme.typography.headlineSmall)
    Text(
        stringResource(R.string.paired_room, workspace.roomLabel),
        style = MaterialTheme.typography.bodyMedium,
    )
    Text(
        if (workspace.hasWorkspace && workspace.projectName != null) {
            stringResource(R.string.paired_project, workspace.projectName!!)
        } else {
            stringResource(R.string.paired_no_workspace)
        },
        style = MaterialTheme.typography.bodyMedium,
    )
    workspace.authenticatedUserId?.let {
        Text(
            stringResource(R.string.paired_user, it),
            style = MaterialTheme.typography.bodyMedium,
        )
    }
    // A desktop that stopped answering has not un-paired: the room, its key and
    // its transport are all still here, so the way out is another ping rather
    // than the connect form. Re-pairing is a separate, manual act because an
    // account room's password is never kept.
    when (liveness) {
        ConnectionLiveness.LIVE -> Unit
        ConnectionLiveness.CHECKING -> Text(
            stringResource(R.string.connection_checking),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        ConnectionLiveness.LOST -> {
            Text(
                stringResource(R.string.connection_lost_detail),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
            )
            TextButton(
                onClick = onVerify,
                modifier = Modifier.testTag(CONNECTION_RETRY_TEST_TAG),
            ) { Text(stringResource(R.string.connection_check_again)) }
        }
    }
    Button(onClick = onDisconnect, modifier = Modifier.fillMaxWidth()) {
        Text(stringResource(R.string.pairing_disconnect))
    }
}
