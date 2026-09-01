package com.bitfun.mobile.app.ui.shell.sidebar

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.remote.SessionActionPopup
import com.bitfun.mobile.app.ui.remote.SessionActionSheet
import com.bitfun.mobile.app.ui.remote.SessionDetailsSheet
import com.bitfun.mobile.core.feature.account.AccountDeviceUi
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.connection.RemoteControlSource
import com.bitfun.mobile.core.feature.session.SessionActionPolicy
import com.bitfun.mobile.core.feature.session.SessionActionScope
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.layout.SettingsPlacement
import com.bitfun.mobile.core.feature.shell.RemoteSidebarSessionRow
import com.bitfun.mobile.core.feature.shell.SidebarPresentation
import com.bitfun.mobile.core.feature.shell.SidebarSessionRow
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState

internal const val SIDEBAR_TEST_TAG: String = "app-sidebar"

/** Tagged because "Code" also labels the session-list filter on the remote screen. */
internal const val SIDEBAR_CODE_TEST_TAG: String = "app-sidebar-code"

/**
 * The drawer, ported from `pages/components/AppSidebar.ets`.
 *
 * The header, the source nav row and the footer are the same in every state so
 * that signing in, or switching what the content area shows, never moves the
 * shared chrome. Which half of the header and footer renders is decided by
 * [accountUserId], exactly as `isAccountAuthenticated` decides it there.
 *
 * The per-row menu is hoisted here rather than into each row: only one row's menu
 * can be open at a time, and holding that as one nullable id is what lets the
 * open row stay highlighted underneath the sheet.
 */
@Composable
internal fun AppSidebar(
    permanent: Boolean,
    sessionDetailsPlacement: SettingsPlacement,
    accountUserId: String?,
    connectionPhase: ConnectionPhase,
    remoteControlSource: RemoteControlSource,
    remoteDevices: List<AccountDeviceUi>,
    remoteSelectedDeviceId: String?,
    remoteDeviceName: String,
    remoteState: RemoteSessionUiState,
    workspaceState: RemoteWorkspaceUiState,
    remoteActive: Boolean,
    remoteSelectedSessionId: String?,
    sessions: List<SidebarSessionRow>,
    selectedSessionId: String?,
    query: String,
    searchOpen: Boolean,
    onQueryChange: (String) -> Unit,
    onToggleSearch: () -> Unit,
    onScanDesktop: () -> Unit,
    onRetryRemoteDevice: () -> Unit,
    onSelectRemoteDevice: (String) -> Unit,
    onOpenRemoteSession: (String) -> Unit,
    onCreateRemoteInWorkspace: (String) -> Unit,
    onOpenRemoteWorkspace: (String) -> Unit,
    onNewChat: () -> Unit,
    onOpenSession: (SidebarSessionRow) -> Unit,
    onArchiveSession: (String, Boolean) -> Unit,
    onExportSession: (SidebarSessionRow) -> Unit,
    onDeleteSession: (String) -> Unit,
    onDeleteRemoteSession: (String) -> Unit,
    onOpenSettings: () -> Unit,
    onOpenAccount: () -> Unit,
    modifier: Modifier,
) {
    val signedIn = !accountUserId.isNullOrBlank()
    val sections = remember(sessions, query) { SidebarPresentation.sections(sessions, query) }

    // Ids rather than rows: the list behind these sheets keeps updating while
    // they are open, and a captured row would go stale the moment a reply lands.
    var actionSessionId by rememberSaveable { mutableStateOf<String?>(null) }
    var actionAnchor by remember { mutableStateOf(IntRect.Zero) }
    var detailsSessionId by rememberSaveable { mutableStateOf<String?>(null) }
    var remoteActionSession by remember { mutableStateOf<RemoteSidebarSessionRow?>(null) }
    var remoteActionAnchor by remember { mutableStateOf(IntRect.Zero) }
    var remoteDetailsSessionId by rememberSaveable { mutableStateOf<String?>(null) }
    var archivedExpanded by rememberSaveable { mutableStateOf(false) }

    Box(modifier = modifier.fillMaxSize().testTag(SIDEBAR_TEST_TAG)) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(start = 20.dp, end = 20.dp, top = 4.dp, bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (signedIn) {
                SidebarAuthenticatedHeader(searchOpen, query, onQueryChange, onToggleSearch)
            } else {
                SidebarSignedOutHeader(onNewChat)
            }

            SidebarSessionList(
                sections = sections,
                selectedSessionId = selectedSessionId,
                activeActionSessionId = actionSessionId,
                searching = query.isNotBlank(),
                archivedExpanded = archivedExpanded,
                onToggleArchived = { archivedExpanded = !archivedExpanded },
                onOpenSession = onOpenSession,
                onOpenActions = { session, anchor ->
                    actionAnchor = anchor
                    actionSessionId = session.id
                },
                workspaceContent = {
                    SidebarRemoteWorkspaceSection(
                        connectionPhase = connectionPhase,
                        controlSource = remoteControlSource,
                        devices = remoteDevices,
                        selectedDeviceId = remoteSelectedDeviceId,
                        deviceName = remoteDeviceName,
                        remoteState = remoteState,
                        workspaceState = workspaceState,
                        selectedSessionId = remoteSelectedSessionId.takeIf { remoteActive },
                        onConnect = onScanDesktop,
                        onRetryActive = onRetryRemoteDevice,
                        onSelectDevice = onSelectRemoteDevice,
                        onOpenSession = onOpenRemoteSession,
                        onOpenActions = { row, anchor ->
                            remoteActionAnchor = anchor
                            remoteActionSession = row
                        },
                        onCreateInWorkspace = onCreateRemoteInWorkspace,
                        onOpenWorkspace = onOpenRemoteWorkspace,
                    )
                },
                footerRoom = if (!signedIn && connectionPhase != ConnectionPhase.CONNECTED) {
                    142.dp
                } else {
                    84.dp
                },
                modifier = Modifier.weight(1f),
            )
        }

        // Over the list, not after it: the 84dp tail the list reserves is what
        // keeps the last conversation from ending up underneath this.
        Box(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .padding(start = 20.dp, end = 20.dp, bottom = 16.dp),
        ) {
            if (signedIn) {
                SidebarAuthenticatedFooter(onNewChat, onOpenSettings)
            } else {
                SidebarSignedOutFooter(
                    showScan = connectionPhase != ConnectionPhase.CONNECTED,
                    onScanDesktop = onScanDesktop,
                    onOpenAccount = onOpenAccount,
                )
            }
        }
    }

    actionSessionId?.let { id ->
        val session = sessions.firstOrNull { it.id == id }
        if (session == null) {
            actionSessionId = null
            return@let
        }
        val actionSurface: @Composable () -> Unit = {
            if (permanent) {
                SessionActionPopup(
                    anchorBounds = actionAnchor,
                    title = session.title,
                    status = session.status,
                    capabilities = SessionActionPolicy.resolve(
                        SessionActionScope.GENERAL,
                        GENERAL_CHAT_AGENT_TYPE,
                        false,
                    ),
                    onViewDetails = { detailsSessionId = id },
                    onArchive = { onArchiveSession(id, !session.status.equals(ARCHIVED, ignoreCase = true)) },
                    onExport = { onExportSession(session) },
                    onDelete = { onDeleteSession(id) },
                    onDismiss = { actionSessionId = null },
                )
            } else {
                SessionActionSheet(
            title = session.title,
            status = session.status,
            // Every sidebar row is a local general chat, so the policy is asked
            // with that agent type rather than one carried on the row.
            capabilities = SessionActionPolicy.resolve(
                SessionActionScope.GENERAL,
                GENERAL_CHAT_AGENT_TYPE,
                false,
            ),
            onViewDetails = { detailsSessionId = id },
            onArchive = {
                onArchiveSession(id, !session.status.equals(ARCHIVED, ignoreCase = true))
            },
            onExport = { onExportSession(session) },
            onDelete = { onDeleteSession(id) },
            onDismiss = { actionSessionId = null },
                )
            }
        }
        actionSurface()
    }

    remoteActionSession?.let { session ->
        val busy = (remoteState as? RemoteSessionUiState.Ready)?.busy == true
        val capabilities = SessionActionPolicy.resolve(
            SessionActionScope.REMOTE,
            session.agentType,
            busy,
        )
        if (permanent) {
            SessionActionPopup(
                anchorBounds = remoteActionAnchor,
                title = session.title,
                status = "",
                capabilities = capabilities,
                onViewDetails = { remoteDetailsSessionId = session.id },
                onDelete = { onDeleteRemoteSession(session.id) },
                onDismiss = { remoteActionSession = null },
            )
        } else {
            SessionActionSheet(
                title = session.title,
                status = "",
                capabilities = capabilities,
                onViewDetails = { remoteDetailsSessionId = session.id },
                onDelete = { onDeleteRemoteSession(session.id) },
                onDismiss = { remoteActionSession = null },
            )
        }
    }

    remoteDetailsSessionId?.let { id ->
        val session = (remoteState as? RemoteSessionUiState.Ready)?.sessions
            ?.firstOrNull { it.id == id }
        if (session == null) {
            remoteDetailsSessionId = null
            return@let
        }
        SessionDetailsSheet(
            title = session.title,
            agentType = session.agentType,
            status = session.status,
            workspaceName = session.workspaceName,
            workspacePath = session.workspacePath,
            createdAt = session.createdAt,
            updatedAt = session.updatedAt,
            messageCount = session.messageCount,
            placement = sessionDetailsPlacement,
            onDismiss = { remoteDetailsSessionId = null },
        )
    }

    detailsSessionId?.let { id ->
        val session = sessions.firstOrNull { it.id == id }
        if (session == null) {
            detailsSessionId = null
            return@let
        }
        SessionDetailsSheet(
            title = session.title,
            agentType = stringResource(R.string.session_group_chat),
            status = session.status,
            // A locally stored conversation has no desktop workspace behind it.
            workspaceName = null,
            workspacePath = null,
            createdAt = session.createdAt,
            updatedAt = session.updatedAt,
            messageCount = session.messageCount,
            placement = sessionDetailsPlacement,
            onDismiss = { detailsSessionId = null },
        )
    }

}

private const val GENERAL_CHAT_AGENT_TYPE = "general_chat"
private const val ARCHIVED = "archived"
