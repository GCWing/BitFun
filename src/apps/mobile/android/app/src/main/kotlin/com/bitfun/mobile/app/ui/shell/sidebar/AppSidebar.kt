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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.remote.SessionActionSheet
import com.bitfun.mobile.app.ui.remote.SessionDetailsSheet
import com.bitfun.mobile.app.ui.session.RenameSessionDialog
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.session.SessionActionPolicy
import com.bitfun.mobile.core.feature.session.SessionActionScope
import com.bitfun.mobile.core.feature.shell.SidebarPresentation
import com.bitfun.mobile.core.feature.shell.SidebarSessionRow

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
    accountUserId: String?,
    connectionPhase: ConnectionPhase,
    remoteActive: Boolean,
    sessions: List<SidebarSessionRow>,
    selectedSessionId: String?,
    query: String,
    searchOpen: Boolean,
    onQueryChange: (String) -> Unit,
    onToggleSearch: () -> Unit,
    onEnterCode: () -> Unit,
    onNewChat: () -> Unit,
    onOpenSession: (SidebarSessionRow) -> Unit,
    onRenameSession: (String, String) -> Unit,
    onArchiveSession: (String, Boolean) -> Unit,
    onExportSession: (SidebarSessionRow) -> Unit,
    onDeleteSession: (String) -> Unit,
    onOpenSettings: () -> Unit,
    onOpenAccount: () -> Unit,
    modifier: Modifier,
) {
    val signedIn = !accountUserId.isNullOrBlank()
    val sections = remember(sessions, query) { SidebarPresentation.sections(sessions, query) }

    // Ids rather than rows: the list behind these sheets keeps updating while
    // they are open, and a captured row would go stale the moment a reply lands.
    var actionSessionId by rememberSaveable { mutableStateOf<String?>(null) }
    var detailsSessionId by rememberSaveable { mutableStateOf<String?>(null) }
    var renameSessionId by rememberSaveable { mutableStateOf<String?>(null) }
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

            NavRow(
                label = stringResource(R.string.sidebar_code),
                active = remoteActive,
                leading = { ConnectionDot(connectionPhase) },
                onClick = onEnterCode,
                modifier = Modifier.testTag(SIDEBAR_CODE_TEST_TAG),
            )

            SidebarSessionList(
                sections = sections,
                selectedSessionId = selectedSessionId,
                activeActionSessionId = actionSessionId,
                searching = query.isNotBlank(),
                archivedExpanded = archivedExpanded,
                onToggleArchived = { archivedExpanded = !archivedExpanded },
                onOpenSession = onOpenSession,
                onOpenActions = { actionSessionId = it.id },
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
                SidebarSignedOutFooter(onOpenAccount)
            }
        }
    }

    actionSessionId?.let { id ->
        val session = sessions.firstOrNull { it.id == id }
        if (session == null) {
            actionSessionId = null
            return@let
        }
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
            onRename = { renameSessionId = id },
            onViewDetails = { detailsSessionId = id },
            onArchive = {
                onArchiveSession(id, !session.status.equals(ARCHIVED, ignoreCase = true))
            },
            onExport = { onExportSession(session) },
            onDelete = { onDeleteSession(id) },
            onDismiss = { actionSessionId = null },
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
            onDismiss = { detailsSessionId = null },
        )
    }

    renameSessionId?.let { id ->
        val session = sessions.firstOrNull { it.id == id }
        if (session == null) {
            renameSessionId = null
            return@let
        }
        RenameSessionDialog(
            currentTitle = session.title,
            onConfirm = { onRenameSession(id, it) },
            onDismiss = { renameSessionId = null },
        )
    }
}

/** A destination, drawn the way `NavRow` draws it in the source. */
@Composable
private fun NavRow(
    label: String,
    active: Boolean,
    leading: @Composable () -> Unit,
    onClick: () -> Unit,
    modifier: Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .height(46.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(if (active) MaterialTheme.colorScheme.surfaceVariant else Color.Transparent)
            .clickable(onClick = onClick)
            .padding(start = 12.dp, end = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        leading()
        Text(
            label,
            fontSize = 18.sp,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
        Icon(
            painterResource(R.drawable.ic_symbol_desktop),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(20.dp),
        )
    }
}

private const val GENERAL_CHAT_AGENT_TYPE = "general_chat"
private const val ARCHIVED = "archived"
