package com.openbitfun.mobile.app.ui.remote

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.app.ui.chat.ConversationView
import com.openbitfun.mobile.app.ui.common.CircleControl
import com.openbitfun.mobile.app.ui.shell.MENU_TEST_TAG
import com.openbitfun.mobile.app.viewmodel.PairingViewModel
import com.openbitfun.mobile.core.feature.connection.ConnectionPhase
import com.openbitfun.mobile.core.feature.layout.SettingsPlacement
import com.openbitfun.mobile.core.feature.connection.connectionPhase
import com.openbitfun.mobile.core.feature.pairing.ConnectionLiveness
import com.openbitfun.mobile.core.feature.pairing.PairedWorkspace
import com.openbitfun.mobile.core.feature.pairing.PairingIntent
import com.openbitfun.mobile.core.feature.pairing.PairingUiState
import com.openbitfun.mobile.core.feature.session.ConversationHeaderPresenter
import com.openbitfun.mobile.core.feature.session.RemoteSessionUiState
import com.openbitfun.mobile.core.feature.workspace.RemoteWorkspaceIntent
import com.openbitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState

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
    settingsPlacement: SettingsPlacement,
    sessionDetailsPlacement: SettingsPlacement,
    viewSettingsPlacement: SettingsPlacement,
    onOpenRemoteSettings: () -> Unit,
    onOpenSidebar: (() -> Unit)? = null,
    onBack: () -> Unit = {},
    onOpenAccount: () -> Unit = {},
    compact: Boolean = true,
    requestedSessionId: String? = null,
    creatingSession: Boolean = false,
    onOpenSession: (String) -> Unit = {},
    onCreateSession: () -> Unit = {},
    onRemoteHome: () -> Unit = {},
    startScanning: Boolean = false,
    onScanStarted: () -> Unit = {},
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
                settingsPlacement = settingsPlacement,
                sessionDetailsPlacement = sessionDetailsPlacement,
                viewSettingsPlacement = viewSettingsPlacement,
                onOpenRemoteSettings = onOpenRemoteSettings,
                deviceId = current.workspace.roomLabel,
                createDevices = emptyList(),
                desktopName = "",
                onCreateDevicePick = {},
                onSessionIntent = viewModel::dispatchSession,
                onWorkspaceIntent = viewModel::dispatchWorkspace,
                onOpenSidebar = onOpenSidebar,
                compact = compact,
                requestedSessionId = requestedSessionId,
                creatingSession = creatingSession,
                onOpenSession = onOpenSession,
                onCreateSession = onCreateSession,
                onRemoteHome = onRemoteHome,
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
            onBack = onBack,
            onOpenAccount = onOpenAccount,
            startScanning = startScanning,
            onScanStarted = onScanStarted,
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
    createDevices: List<CreateDeviceChoice>,
    accountUsername: String,
    phase: ConnectionPhase,
    settingsPlacement: SettingsPlacement,
    sessionDetailsPlacement: SettingsPlacement,
    viewSettingsPlacement: SettingsPlacement,
    onOpenRemoteSettings: () -> Unit,
    onCreateDevicePick: (String) -> Unit,
    onSessionIntent: (com.openbitfun.mobile.core.feature.session.RemoteSessionIntent) -> Unit,
    onWorkspaceIntent: (RemoteWorkspaceIntent) -> Unit,
    onOpenSidebar: (() -> Unit)? = null,
    compact: Boolean = true,
    requestedSessionId: String? = null,
    creatingSession: Boolean = false,
    onOpenSession: (String) -> Unit = {},
    onCreateSession: () -> Unit = {},
    onRemoteHome: () -> Unit = {},
    modifier: Modifier,
) {
    RemoteConnectedScreen(
        remoteState = remoteState,
        workspaceState = workspaceState,
        phase = phase,
        settingsPlacement = settingsPlacement,
        sessionDetailsPlacement = sessionDetailsPlacement,
        viewSettingsPlacement = viewSettingsPlacement,
        onOpenRemoteSettings = onOpenRemoteSettings,
        deviceId = deviceId,
        createDevices = createDevices,
        desktopName = deviceName,
        onCreateDevicePick = onCreateDevicePick,
        onSessionIntent = onSessionIntent,
        onWorkspaceIntent = onWorkspaceIntent,
        onOpenSidebar = onOpenSidebar,
        compact = compact,
        requestedSessionId = requestedSessionId,
        creatingSession = creatingSession,
        onOpenSession = onOpenSession,
        onCreateSession = onCreateSession,
        onRemoteHome = onRemoteHome,
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
    settingsPlacement: SettingsPlacement,
    sessionDetailsPlacement: SettingsPlacement,
    viewSettingsPlacement: SettingsPlacement,
    onOpenRemoteSettings: () -> Unit,
    deviceId: String,
    createDevices: List<CreateDeviceChoice>,
    desktopName: String,
    onCreateDevicePick: (String) -> Unit,
    onSessionIntent: (com.openbitfun.mobile.core.feature.session.RemoteSessionIntent) -> Unit,
    onWorkspaceIntent: (RemoteWorkspaceIntent) -> Unit,
    onOpenSidebar: (() -> Unit)?,
    compact: Boolean,
    requestedSessionId: String?,
    creatingSession: Boolean,
    onOpenSession: (String) -> Unit,
    onCreateSession: () -> Unit,
    onRemoteHome: () -> Unit,
    connectionDetails: @Composable () -> Unit,
    modifier: Modifier,
) {
    RemoteDownloadSaver(workspaceState, onWorkspaceIntent)
    val conversation = (remoteState as? RemoteSessionUiState.Ready)?.takeIf {
        requestedSessionId != null && it.selectedSessionId == requestedSessionId && it.timeline != null
    }
    if (conversation != null) {
        ConversationView(
            state = conversation,
            phase = phase,
            settingsPlacement = settingsPlacement,
            onBack = onRemoteHome,
            onOpenSidebar = onOpenSidebar,
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
            download = (workspaceState as? RemoteWorkspaceUiState.Ready)?.download
                ?: com.openbitfun.mobile.core.feature.workspace.RemoteFileDownloadUiState.None,
            onDownloadFile = { path, label ->
                onWorkspaceIntent(
                    RemoteWorkspaceIntent.DownloadFile(
                        path,
                        label,
                        conversation.selectedSessionId.orEmpty(),
                    ),
                )
            },
            modifier = modifier,
        )
    } else if (creatingSession) {
        CreateSessionRoute(
            sessionState = remoteState,
            workspaceState = workspaceState,
            phase = phase,
            deviceId = deviceId,
            devices = createDevices,
            compact = compact,
            onDevicePick = onCreateDevicePick,
            onBack = onRemoteHome,
            onCreated = onOpenSession,
            onWorkspaceIntent = onWorkspaceIntent,
            onIntent = onSessionIntent,
            modifier = modifier,
        )
    } else if (compact) {
        RemoteCompactHome(
            remoteState = remoteState,
            desktopName = desktopName,
            onOpenSidebar = onOpenSidebar,
            onCreate = onCreateSession,
            onOpenRemoteSettings = onOpenRemoteSettings,
            modifier = modifier,
        )
    } else {
        Column(modifier = modifier.fillMaxSize()) {
            RemoteShellHeader(onOpenSidebar, onOpenRemoteSettings = onOpenRemoteSettings)
            RemoteSessionListView(
                state = remoteState,
                workspaceState = workspaceState,
                compact = compact,
                sessionDetailsPlacement = sessionDetailsPlacement,
                viewSettingsPlacement = viewSettingsPlacement,
                connectionDetails = connectionDetails,
                onIntent = onSessionIntent,
                onWorkspaceIntent = onWorkspaceIntent,
                onOpen = onOpenSession,
                onCreate = onCreateSession,
                modifier = Modifier.weight(1f),
            )
        }
    }
}

@Composable
private fun RemoteCompactHome(
    remoteState: RemoteSessionUiState,
    desktopName: String,
    onOpenSidebar: (() -> Unit)?,
    onCreate: () -> Unit,
    onOpenRemoteSettings: () -> Unit,
    modifier: Modifier,
) {
    val ready = remoteState as? RemoteSessionUiState.Ready
    val hasSessions = ready?.sessions?.isNotEmpty() == true
    val loading = remoteState is RemoteSessionUiState.Loading
    val title = when {
        loading -> stringResource(R.string.sessions_loading)
        remoteState is RemoteSessionUiState.Failed -> stringResource(R.string.sessions_failed)
        hasSessions -> stringResource(R.string.remote_pick_session)
        else -> stringResource(R.string.remote_empty_title)
    }
    val body = if (hasSessions) {
        stringResource(R.string.remote_pick_session_text)
    } else {
        stringResource(R.string.remote_empty_text)
    }

    Column(modifier = modifier.fillMaxSize()) {
        RemoteShellHeader(onOpenSidebar, desktopName, onOpenRemoteSettings)
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .padding(start = 24.dp, end = 24.dp, bottom = 56.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            if (loading) {
                CircularProgressIndicator(
                    modifier = Modifier.padding(bottom = 18.dp).size(28.dp),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Text(
                title,
                fontSize = 20.sp,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onSurface,
                textAlign = TextAlign.Center,
            )
            Text(
                body,
                fontSize = 14.sp,
                lineHeight = 21.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.width(280.dp).padding(top = 10.dp),
            )
            if (!loading) {
                Button(
                    onClick = onCreate,
                    shape = RoundedCornerShape(23.dp),
                    modifier = Modifier.padding(top = 12.dp).width(148.dp).height(46.dp),
                ) {
                    Text(stringResource(R.string.remote_start_session), fontSize = 15.sp)
                }
            }
        }
    }
}

@Composable
private fun RemoteShellHeader(
    onOpenSidebar: (() -> Unit)?,
    subtitle: String = "",
    onOpenRemoteSettings: () -> Unit,
) {
    val hasSubtitle = subtitle.isNotBlank()
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(if (hasSubtitle) 76.dp else 64.dp)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        onOpenSidebar?.let { openSidebar ->
            CircleControl(
                icon = R.drawable.ic_symbol_menu_lines,
                glyphSize = 22,
                contentDescription = stringResource(R.string.shell_open_sidebar),
                onClick = openSidebar,
                modifier = Modifier.testTag(MENU_TEST_TAG),
            )
        } ?: Box(Modifier.size(44.dp))
        Column(
            modifier = Modifier.weight(1f),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            Text(
                stringResource(R.string.navigation_remote),
                fontSize = if (hasSubtitle) 18.sp else 17.sp,
                fontWeight = FontWeight.Medium,
                maxLines = 1,
                textAlign = TextAlign.Center,
            )
            if (hasSubtitle) {
                Text(
                    subtitle,
                    fontSize = 14.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    textAlign = TextAlign.Center,
                )
            }
        }
        CircleControl(
            icon = R.drawable.ic_symbol_gearshape,
            glyphSize = 19,
            contentDescription = stringResource(R.string.remote_settings_title),
            onClick = onOpenRemoteSettings,
            modifier = Modifier,
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
                // The pane sizes itself to the space it is given, and this
                // panel is inside a scrolling column with none to give.
                FilePreviewSurface(
                    preview = state.preview,
                    download = state.download,
                    remoteAvailable = true,
                    onIntent = onIntent,
                    modifier = Modifier.fillMaxWidth().height(360.dp),
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
