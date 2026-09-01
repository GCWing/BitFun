package com.bitfun.mobile.app.ui.shell.sidebar

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.core.feature.account.AccountDeviceUi
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.connection.ConnectionStatusPresenter
import com.bitfun.mobile.core.feature.connection.RemoteControlSource
import com.bitfun.mobile.core.feature.shell.RemoteSidebarSessionRow
import com.bitfun.mobile.core.feature.shell.RemoteSidebarWorkspaceRow
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import com.bitfun.mobile.app.ui.theme.bitFunColors

private const val SESSIONS_PER_WORKSPACE = 3
private const val WORKSPACES_PER_BATCH = 3
private const val DEVICES_PER_BATCH = 3
internal const val SIDEBAR_REMOTE_SESSION_TEST_TAG: String = "app-sidebar-remote-session"

/** The remote workspace tree appended under local conversations in Harmony's unified sidebar. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun SidebarRemoteWorkspaceSection(
    connectionPhase: ConnectionPhase,
    controlSource: RemoteControlSource,
    devices: List<AccountDeviceUi>,
    selectedDeviceId: String?,
    deviceName: String,
    remoteState: RemoteSessionUiState,
    workspaceState: RemoteWorkspaceUiState,
    selectedSessionId: String?,
    onConnect: () -> Unit,
    onRetryActive: () -> Unit,
    onSelectDevice: (String) -> Unit,
    onOpenSession: (String) -> Unit,
    onOpenActions: (RemoteSidebarSessionRow, IntRect) -> Unit,
    onCreateInWorkspace: (String) -> Unit,
    onOpenWorkspace: (String) -> Unit,
) {
    val connected = ConnectionStatusPresenter.canReachSessions(connectionPhase)
    val addConnectionLabel = stringResource(R.string.sidebar_add_connection)
    val transientDeviceKey = remember(deviceName) { "qr:$deviceName" }
    val projectedDevices = remember(devices, controlSource, deviceName) {
        if (
            controlSource == RemoteControlSource.QR_PAIRING &&
            deviceName.isNotBlank() &&
            devices.none { it.name == deviceName }
        ) {
            listOf(AccountDeviceUi(transientDeviceKey, deviceName, true, null)) + devices
        } else {
            devices
        }
    }
    val activeDeviceId = when (controlSource) {
        RemoteControlSource.QR_PAIRING -> projectedDevices.firstOrNull {
            it.id == transientDeviceKey || it.name == deviceName
        }?.id
        RemoteControlSource.ACCOUNT_DEVICE -> selectedDeviceId
        RemoteControlSource.NONE -> null
    }
    var expandedDeviceIds by rememberSaveable { mutableStateOf(emptyList<String>()) }
    var visibleDeviceCount by rememberSaveable { mutableStateOf(DEVICES_PER_BATCH) }
    var cachedRemoteStates by remember {
        mutableStateOf(emptyMap<String, RemoteSessionUiState.Ready>())
    }
    var cachedWorkspaceStates by remember {
        mutableStateOf(emptyMap<String, RemoteWorkspaceUiState.Ready>())
    }

    LaunchedEffect(activeDeviceId, remoteState, workspaceState) {
        activeDeviceId?.let { id ->
            if (id !in expandedDeviceIds) expandedDeviceIds = expandedDeviceIds + id
            (remoteState as? RemoteSessionUiState.Ready)?.let { ready ->
                cachedRemoteStates = cachedRemoteStates + (id to ready)
            }
            (workspaceState as? RemoteWorkspaceUiState.Ready)?.let { ready ->
                cachedWorkspaceStates = cachedWorkspaceStates + (id to ready)
            }
        }
    }

    Column(modifier = Modifier.fillMaxWidth().padding(top = 18.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth().height(38.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                stringResource(R.string.sidebar_devices),
                fontSize = 14.sp,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Box(Modifier.weight(1f))
            if (!connected && projectedDevices.isEmpty()) {
                Text(
                    stringResource(R.string.sidebar_workspaces_offline),
                    fontSize = 12.sp,
                    color = MaterialTheme.colorScheme.outline,
                    modifier = Modifier.padding(end = 8.dp),
                )
            }
            Box(
                modifier = Modifier
                    .size(32.dp)
                    .clip(CircleShape)
                    .clickable(role = Role.Button, onClick = onConnect)
                    .semantics(mergeDescendants = true) {
                        contentDescription = addConnectionLabel
                    },
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    painterResource(R.drawable.ic_symbol_plus),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(17.dp),
                )
            }
        }

        if (projectedDevices.isEmpty()) {
            SidebarActiveDeviceBody(
                connected = connected,
                loading = remoteState is RemoteSessionUiState.Loading ||
                    workspaceState is RemoteWorkspaceUiState.Loading,
                failed = remoteState is RemoteSessionUiState.Failed ||
                    workspaceState is RemoteWorkspaceUiState.Failed,
                deviceKey = deviceName,
                remoteState = remoteState as? RemoteSessionUiState.Ready,
                workspaceState = workspaceState as? RemoteWorkspaceUiState.Ready,
                selectedSessionId = selectedSessionId,
                onConnect = onConnect,
                onRetry = onRetryActive,
                onOpenSession = onOpenSession,
                onOpenActions = onOpenActions,
                canActOnSessions = true,
                onCreateInWorkspace = onCreateInWorkspace,
                onOpenWorkspace = onOpenWorkspace,
            )
        } else {
            projectedDevices.take(visibleDeviceCount).forEach { device ->
                val expanded = device.id in expandedDeviceIds
                val active = device.id == activeDeviceId
                val transient = controlSource == RemoteControlSource.QR_PAIRING &&
                    device.id == activeDeviceId
                val cachedRemote = cachedRemoteStates[device.id]
                val cachedWorkspace = cachedWorkspaceStates[device.id]
                val shownRemote = if (active) {
                    (remoteState as? RemoteSessionUiState.Ready) ?: cachedRemote
                } else {
                    cachedRemote
                }
                val shownWorkspace = if (active) {
                    (workspaceState as? RemoteWorkspaceUiState.Ready) ?: cachedWorkspace
                } else {
                    cachedWorkspace
                }
                val loading = active && (
                    remoteState is RemoteSessionUiState.Idle ||
                        remoteState is RemoteSessionUiState.Loading ||
                        workspaceState is RemoteWorkspaceUiState.Idle ||
                        workspaceState is RemoteWorkspaceUiState.Loading
                    )
                val failed = active && (
                    remoteState is RemoteSessionUiState.Failed ||
                        workspaceState is RemoteWorkspaceUiState.Failed
                    )
                SidebarDeviceHeader(
                    deviceName = device.name.ifBlank { device.id },
                    online = device.online,
                    expanded = expanded,
                    loading = loading,
                    onToggle = {
                        if (active) {
                            expandedDeviceIds = if (expanded) {
                                expandedDeviceIds - device.id
                            } else {
                                expandedDeviceIds + device.id
                            }
                        } else if (device.online && !transient) {
                            activeDeviceId?.let { currentId ->
                                (remoteState as? RemoteSessionUiState.Ready)?.let { ready ->
                                    cachedRemoteStates = cachedRemoteStates + (currentId to ready)
                                }
                                (workspaceState as? RemoteWorkspaceUiState.Ready)?.let { ready ->
                                    cachedWorkspaceStates = cachedWorkspaceStates + (currentId to ready)
                                }
                            }
                            if (!expanded) expandedDeviceIds = expandedDeviceIds + device.id
                            onSelectDevice(device.id)
                        }
                    },
                )
                if (expanded) {
                    SidebarActiveDeviceBody(
                        connected = if (active) connected else shownWorkspace != null,
                        loading = loading,
                        failed = failed,
                        deviceKey = device.id,
                        remoteState = shownRemote,
                        workspaceState = shownWorkspace,
                        selectedSessionId = selectedSessionId,
                        onConnect = onConnect,
                        onRetry = { onSelectDevice(device.id) },
                        onOpenSession = onOpenSession,
                        onOpenActions = onOpenActions,
                        canActOnSessions = active,
                        onCreateInWorkspace = onCreateInWorkspace,
                        onOpenWorkspace = onOpenWorkspace,
                    )
                }
            }
            if (visibleDeviceCount < projectedDevices.size) {
                MoreRow(
                    hidden = projectedDevices.size - visibleDeviceCount,
                    startPadding = 10,
                    onClick = { visibleDeviceCount += DEVICES_PER_BATCH },
                    devices = true,
                )
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun SidebarActiveDeviceBody(
    connected: Boolean,
    loading: Boolean,
    failed: Boolean,
    deviceKey: String,
    remoteState: RemoteSessionUiState.Ready?,
    workspaceState: RemoteWorkspaceUiState.Ready?,
    selectedSessionId: String?,
    onConnect: () -> Unit,
    onRetry: () -> Unit,
    onOpenSession: (String) -> Unit,
    onOpenActions: (RemoteSidebarSessionRow, IntRect) -> Unit,
    canActOnSessions: Boolean,
    onCreateInWorkspace: (String) -> Unit,
    onOpenWorkspace: (String) -> Unit,
) {
    val readyWorkspace = workspaceState
    val readySessions = remoteState
    val busy = remoteState?.busy == true
    val entries = remember(readyWorkspace, readySessions) {
        // Keep this projection local: the shared function is off-limits here, and
        // its raw path equality loses sessions when a desktop adds a separator.
        if (readyWorkspace == null) {
            emptyList()
        } else {
            val selected = readyWorkspace.selected
            val workspaceRows = buildList {
                if (selected != null && selected.path.isNotBlank()) {
                    add(selected.path to selected.name)
                }
                readyWorkspace.workspaces.forEach { workspace ->
                    if (workspace.path.isNotBlank() && none {
                        RemoteWorkspacePathPolicy.equal(it.first, workspace.path)
                    }) {
                        add(workspace.path to workspace.name)
                    }
                }
            }
            workspaceRows.map { (path, name) ->
                RemoteSidebarWorkspaceRow(
                    path = path,
                    name = name,
                    selected = RemoteWorkspacePathPolicy.equal(path, selected?.path.orEmpty()),
                    sessions = readySessions?.sessions.orEmpty()
                        .filter { session ->
                            RemoteWorkspacePathPolicy.equal(
                                session.workspacePath ?: selected?.path.orEmpty(),
                                path,
                            )
                        }
                        .map { session ->
                            RemoteSidebarSessionRow(session.id, session.title, session.agentType)
                        },
                )
            }
        }
    }
    var collapsedPaths by rememberSaveable(deviceKey) { mutableStateOf(emptyList<String>()) }
    var expandedSessionPaths by rememberSaveable(deviceKey) { mutableStateOf(emptyList<String>()) }
    var visibleWorkspaceCount by rememberSaveable(deviceKey) {
        mutableStateOf(WORKSPACES_PER_BATCH)
    }

    if (!connected) {
        ConnectDesktopRow(onConnect)
    } else if (failed) {
        DeviceFailedRow(onRetry)
    } else if (loading && entries.isEmpty()) {
        DeviceLoadingRow()
    } else if (entries.isEmpty()) {
        Text(
            stringResource(R.string.sidebar_empty_workspaces),
            fontSize = 14.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.fillMaxWidth().padding(start = 10.dp, top = 6.dp, bottom = 6.dp),
        )
    } else {
        entries.take(visibleWorkspaceCount).forEach { entry ->
            val path = entry.path
            val workspaceSessions = entry.sessions
            val collapsed = path in collapsedPaths
            Column(modifier = Modifier.fillMaxWidth().padding(bottom = 6.dp)) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(46.dp)
                        .clip(RoundedCornerShape(10.dp))
                        .combinedClickable(
                            role = Role.Button,
                            onClick = {
                                collapsedPaths = if (collapsed) {
                                    collapsedPaths - path
                                } else {
                                    collapsedPaths + path
                                }
                            },
                            onLongClick = { onOpenWorkspace(path) },
                        )
                        .semantics(mergeDescendants = true) {
                            contentDescription = entry.name.ifBlank { path.substringAfterLast('/') }
                        }
                        .padding(start = 10.dp, end = 6.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        painterResource(R.drawable.ic_symbol_folder),
                        contentDescription = null,
                        tint = if (entry.selected) {
                            MaterialTheme.colorScheme.onSurface
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                        modifier = Modifier.size(21.dp),
                    )
                    Text(
                        entry.name.ifBlank { path.substringAfterLast('/') },
                        fontSize = 15.sp,
                        fontWeight = if (entry.selected) {
                            FontWeight.Medium
                        } else {
                            FontWeight.Normal
                        },
                        color = MaterialTheme.colorScheme.onSurface,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    Box(
                        modifier = Modifier
                            .size(width = 30.dp, height = 40.dp)
                            .clip(RoundedCornerShape(8.dp))
                            .clickable(role = Role.Button) { onCreateInWorkspace(path) },
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            painterResource(R.drawable.ic_symbol_square_and_pencil),
                            contentDescription = stringResource(R.string.sidebar_new_in_workspace),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                    Icon(
                        painterResource(
                            if (collapsed) R.drawable.ic_symbol_chevron_right
                            else R.drawable.ic_symbol_chevron_down,
                        ),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(13.dp),
                    )
                }

                if (!collapsed) {
                    val limit = if (path in expandedSessionPaths) {
                        workspaceSessions.size
                    } else {
                        SESSIONS_PER_WORKSPACE
                    }
                    workspaceSessions.take(limit).forEach { session ->
                        RemoteSessionRow(
                            session = session,
                            selected = session.id == selectedSessionId,
                            busy = busy,
                            canActOnSessions = canActOnSessions,
                            onOpenSession = onOpenSession,
                            onOpenActions = onOpenActions,
                        )
                    }
                    if (limit < workspaceSessions.size) {
                        MoreRow(
                            hidden = workspaceSessions.size - limit,
                            startPadding = 26,
                            onClick = { expandedSessionPaths = expandedSessionPaths + path },
                        )
                    }
                }
            }
        }
        if (visibleWorkspaceCount < entries.size) {
            MoreRow(
                hidden = entries.size - visibleWorkspaceCount,
                startPadding = 10,
                onClick = { visibleWorkspaceCount += WORKSPACES_PER_BATCH },
                workspaces = true,
            )
        }
        if (loading) DeviceLoadingRow()
    }
}

@Composable
private fun SidebarDeviceHeader(
    deviceName: String,
    online: Boolean,
    expanded: Boolean,
    loading: Boolean,
    onToggle: () -> Unit,
) {
    val deviceLabel = deviceName
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(46.dp)
            .clip(RoundedCornerShape(10.dp))
            .clickable(role = Role.Button, onClick = onToggle)
            .semantics(mergeDescendants = true) {
                contentDescription = deviceLabel
            }
            .padding(start = 10.dp, end = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painterResource(R.drawable.ic_symbol_desktop),
            contentDescription = null,
            tint = if (online) MaterialTheme.colorScheme.onSurface
            else MaterialTheme.colorScheme.outline,
            modifier = Modifier.size(21.dp),
        )
        Text(
            deviceName,
            fontSize = 15.sp,
            fontWeight = FontWeight.Medium,
            color = if (online) MaterialTheme.colorScheme.onSurface
            else MaterialTheme.colorScheme.outline,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        if (!online) {
            Text(
                stringResource(R.string.sidebar_device_offline),
                fontSize = 12.sp,
                color = MaterialTheme.colorScheme.outline,
            )
        }
        if (loading) {
            CircularProgressIndicator(
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                strokeWidth = 1.5.dp,
                modifier = Modifier.size(14.dp),
            )
        }
        Icon(
            painterResource(
                if (expanded) R.drawable.ic_symbol_chevron_down
                else R.drawable.ic_symbol_chevron_right,
            ),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(13.dp),
        )
    }
}

@Composable
private fun DeviceLoadingRow() {
    Row(
        modifier = Modifier.fillMaxWidth().height(40.dp).padding(start = 10.dp, end = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CircularProgressIndicator(
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            strokeWidth = 1.5.dp,
            modifier = Modifier.size(14.dp),
        )
        Text(
            stringResource(R.string.sidebar_device_loading),
            fontSize = 13.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun DeviceFailedRow(onRetry: () -> Unit) {
    val retryLabel = stringResource(R.string.sidebar_device_retry)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(40.dp)
            .padding(start = 10.dp, end = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            stringResource(R.string.sidebar_device_load_failed),
            fontSize = 13.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        Text(
            retryLabel,
            fontSize = 13.sp,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier
                .clickable(role = Role.Button, onClick = onRetry)
                .semantics { contentDescription = retryLabel },
        )
    }
}

@Composable
private fun ConnectDesktopRow(onConnect: () -> Unit) {
    val connectLabel = stringResource(R.string.sidebar_connect_desktop)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(46.dp)
            .clip(RoundedCornerShape(10.dp))
            .clickable(role = Role.Button, onClick = onConnect)
            .semantics(mergeDescendants = true) {
                contentDescription = connectLabel
            }
            .padding(start = 10.dp, end = 8.dp)
            .testTag(SIDEBAR_CODE_TEST_TAG),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painterResource(R.drawable.ic_symbol_desktop),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(22.dp),
        )
        Text(
            connectLabel,
            fontSize = 15.sp,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
        Icon(
            painterResource(R.drawable.ic_symbol_chevron_right),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(13.dp),
        )
    }
}

@Composable
private fun RemoteSessionRow(
    session: RemoteSidebarSessionRow,
    selected: Boolean,
    busy: Boolean,
    canActOnSessions: Boolean,
    onOpenSession: (String) -> Unit,
    onOpenActions: (RemoteSidebarSessionRow, IntRect) -> Unit,
) {
    var anchorBounds by remember { mutableStateOf(IntRect.Zero) }
    val sessionTitle = session.title.ifBlank { stringResource(R.string.sidebar_untitled) }
    val icon = when (session.agentType.lowercase()) {
        "code" -> R.drawable.ic_symbol_code_square
        "claw", "assistant", "chat" -> R.drawable.ic_symbol_message
        else -> R.drawable.ic_symbol_doc_text
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(44.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(if (selected) MaterialTheme.colorScheme.surfaceVariant else bitFunColors.transparent)
            .onGloballyPositioned { coordinates ->
                anchorBounds = coordinates.boundsInWindow().toIntRect()
            }
            .combinedClickable(
                enabled = !busy,
                role = Role.Button,
                onClick = { onOpenSession(session.id) },
                onLongClick = if (canActOnSessions) {
                    { onOpenActions(session, anchorBounds) }
                } else {
                    null
                },
            )
            .semantics(mergeDescendants = true) {
                contentDescription = sessionTitle
            }
            .padding(start = 26.dp, end = 4.dp)
            .testTag(SIDEBAR_REMOTE_SESSION_TEST_TAG),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painterResource(icon),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(19.dp),
        )
        Text(
            sessionTitle,
            fontSize = 15.sp,
            fontWeight = if (selected) FontWeight.Medium else FontWeight.Normal,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        if (canActOnSessions) {
            IconButton(
                enabled = !busy,
                onClick = { onOpenActions(session, anchorBounds) },
            ) {
                Icon(
                    painterResource(R.drawable.ic_symbol_ellipsis),
                    contentDescription = stringResource(R.string.session_actions),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
    }
}

private fun Rect.toIntRect(): IntRect = IntRect(
    left = left.toInt(),
    top = top.toInt(),
    right = right.toInt(),
    bottom = bottom.toInt(),
)

@Composable
private fun MoreRow(
    hidden: Int,
    startPadding: Int,
    onClick: () -> Unit,
    workspaces: Boolean = false,
    devices: Boolean = false,
) {
    val moreLabel = stringResource(
        when {
            devices -> R.string.sidebar_more_devices
            workspaces -> R.string.sidebar_more_workspaces
            else -> R.string.sessions_show_more
        },
        hidden,
    )
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(40.dp)
            .clickable(role = Role.Button, onClick = onClick)
            .semantics(mergeDescendants = true) {
                contentDescription = moreLabel
            }
            .padding(start = startPadding.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            moreLabel,
            fontSize = 13.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
