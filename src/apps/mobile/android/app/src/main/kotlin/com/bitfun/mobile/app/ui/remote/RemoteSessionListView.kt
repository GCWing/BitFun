package com.bitfun.mobile.app.ui.remote

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.state.SessionViewSettings
import com.bitfun.mobile.app.ui.settings.SessionViewSettingsSheet
import com.bitfun.mobile.app.ui.settings.VIEW_SETTINGS_TOGGLE_TEST_TAG
import com.bitfun.mobile.app.ui.settings.statusText
import com.bitfun.mobile.core.feature.session.RelativeTime
import com.bitfun.mobile.core.feature.session.RemoteSessionFailureReason
import com.bitfun.mobile.core.feature.session.RemoteSessionIntent
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.session.SessionActionPolicy
import com.bitfun.mobile.core.feature.session.SessionActionScope
import com.bitfun.mobile.core.feature.session.SessionAgentFilter
import com.bitfun.mobile.core.feature.session.SessionListPresentation
import com.bitfun.mobile.core.feature.session.SessionListSection
import com.bitfun.mobile.core.feature.session.SessionTimePresentation
import com.bitfun.mobile.core.feature.session.SessionWorkspaceContext
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceIntent
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState

internal const val SESSION_LIST_TEST_TAG: String = "session-list"
internal const val SESSION_CREATE_TEST_TAG: String = "session-create"
internal const val SESSION_PROJECTS_TEST_TAG: String = "session-projects"
internal const val SESSION_SHOW_MORE_TEST_TAG_PREFIX: String = "session-show-more:"

/**
 * The paired desktop's sessions, ported from `pages/components/SessionList.ets`.
 *
 * Opening a row hands the screen over to [ConversationView]; this surface is
 * everything that is *about* sessions rather than inside one.
 */
@Composable
internal fun RemoteSessionListView(
    state: RemoteSessionUiState,
    workspaceState: RemoteWorkspaceUiState,
    connectionDetails: @Composable () -> Unit,
    onIntent: (RemoteSessionIntent) -> Unit,
    onWorkspaceIntent: (RemoteWorkspaceIntent) -> Unit,
    onOpen: (String) -> Unit,
    onCreate: () -> Unit,
    modifier: Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp)
            .testTag(SESSION_LIST_TEST_TAG),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        connectionDetails()

        RemoteSessionListContent(
            state = state,
            workspaceState = workspaceState,
            onIntent = onIntent,
            onOpen = onOpen,
            onCreate = onCreate,
        )

        RemoteWorkspacePanel(
            state = workspaceState,
            sessionId = (state as? RemoteSessionUiState.Ready)?.selectedSessionId.orEmpty(),
            onIntent = onWorkspaceIntent,
            // The shell gives the file a pane or the whole page; see
            // `MobileScreen`'s `previewLayout`.
            showPreview = false,
        )
    }
}

/**
 * The session list itself, without a scroll container of its own.
 *
 * Kept separate because the account sheet shows the same list under a different
 * header, and both callers already own the surface they scroll.
 */
@Composable
internal fun RemoteSessionListContent(
    state: RemoteSessionUiState,
    workspaceState: RemoteWorkspaceUiState,
    onIntent: (RemoteSessionIntent) -> Unit,
    onOpen: (String) -> Unit,
    /** Opens the longer create route, where the first message is written. */
    onCreate: () -> Unit,
) {
    var search by rememberSaveable { mutableStateOf("") }
    var renaming by rememberSaveable { mutableStateOf<String?>(null) }
    var renameDraft by rememberSaveable { mutableStateOf("") }
    var actionsFor by rememberSaveable { mutableStateOf<String?>(null) }
    var detailsFor by rememberSaveable { mutableStateOf<String?>(null) }
    var viewSettingsOpen by rememberSaveable { mutableStateOf(false) }
    var collapsedSectionKeys by rememberSaveable { mutableStateOf<List<String>>(emptyList()) }
    var revealedSectionKeys by rememberSaveable { mutableStateOf<List<String>>(emptyList()) }
    // Not saveable, like the delete confirmation: an open menu is a finger
    // half-way through a gesture, not a place to come back to.
    var createMenuOpen by remember { mutableStateOf(false) }
    var viewSettings by rememberSaveable(stateSaver = SessionViewSettings.Saver) {
        mutableStateOf(SessionViewSettings.Default)
    }

    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(stringResource(R.string.sessions_title), style = MaterialTheme.typography.titleLarge)
            if (state is RemoteSessionUiState.Ready && !state.busy) {
                Text(
                    stringResource(R.string.sessions_messages_synced),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        when (state) {
            RemoteSessionUiState.Idle -> Unit
            RemoteSessionUiState.Loading -> Text(stringResource(R.string.sessions_loading))
            is RemoteSessionUiState.Failed -> SessionFailure(
                state,
                onRetry = { onIntent(RemoteSessionIntent.Load) },
            )
            is RemoteSessionUiState.Ready -> {
                // Search is applied on tap, not per keystroke: every apply is a
                // round trip to the desktop.
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(
                        value = search,
                        onValueChange = { search = it },
                        label = { Text(stringResource(R.string.sessions_search)) },
                        enabled = !state.busy,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(
                        onClick = { onIntent(RemoteSessionIntent.Search(search)) },
                        enabled = !state.busy,
                    ) { Text(stringResource(R.string.sessions_search_action)) }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(
                        SessionAgentFilter.ALL to R.string.sessions_filter_all,
                        SessionAgentFilter.CODE to R.string.sessions_filter_code,
                        SessionAgentFilter.COWORK to R.string.sessions_filter_cowork,
                    ).forEach { (filter, label) ->
                        FilterChip(
                            selected = state.agentFilter == filter,
                            onClick = { onIntent(RemoteSessionIntent.SetAgentFilter(filter)) },
                            enabled = !state.busy,
                            label = { Text(stringResource(label)) },
                        )
                    }
                }
                // Icons rather than three sentences side by side, as
                // `RemoteSessionList.ets` has them: the source puts creation
                // behind a pencil that opens a Code/Cowork menu, and on a phone
                // the spelled-out labels wrapped mid-word.
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box {
                        IconButton(
                            onClick = { createMenuOpen = true },
                            enabled = !state.busy,
                        ) {
                            Icon(
                                painterResource(R.drawable.ic_symbol_square_and_pencil),
                                contentDescription = stringResource(R.string.sidebar_new_chat),
                                modifier = Modifier.size(21.dp),
                            )
                        }
                        // The two agent types the desktop can start, named as
                        // `ProjectCreateMenu` names them, above the longer route
                        // that lets the user say what the session is for first.
                        DropdownMenu(
                            expanded = createMenuOpen,
                            onDismissRequest = { createMenuOpen = false },
                        ) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.create_title)) },
                                onClick = {
                                    createMenuOpen = false
                                    onCreate()
                                },
                                modifier = Modifier.testTag(SESSION_CREATE_TEST_TAG),
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.sessions_filter_code)) },
                                onClick = {
                                    createMenuOpen = false
                                    onIntent(RemoteSessionIntent.CreateSession("code"))
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.sessions_filter_cowork)) },
                                onClick = {
                                    createMenuOpen = false
                                    onIntent(RemoteSessionIntent.CreateSession("cowork"))
                                },
                            )
                        }
                    }
                    IconButton(
                        onClick = { viewSettingsOpen = !viewSettingsOpen },
                        modifier = Modifier.testTag(VIEW_SETTINGS_TOGGLE_TEST_TAG),
                    ) {
                        Icon(
                            // The source marks this control with the same three
                            // dots the session overflow uses — `AppSidebar.ets`
                            // renders `SidebarGlyph({ kind: 'session_more' })`
                            // here — so a tuner slider would be our invention.
                            painterResource(R.drawable.ic_symbol_ellipsis),
                            contentDescription = stringResource(R.string.view_settings_title),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }

                // The desktop's search is a separate, server-side narrowing; the
                // sheet's filters are applied here on what came back, exactly as
                // `RemoteSessionList.ets` layers the two.
                val workspace = workspaceState.asSessionContext()
                val view = remember(state.sessions, workspace, viewSettings) {
                    SessionListPresentation.view(
                        sessions = state.sessions,
                        workspace = workspace,
                        options = viewSettings.options(query = ""),
                        nowMs = System.currentTimeMillis(),
                    )
                }

                if (viewSettingsOpen) {
                    SessionViewSettingsSheet(
                        settings = viewSettings,
                        workspaces = remember(state.sessions, workspace) {
                            SessionListPresentation.workspaceOptions(state.sessions, workspace)
                        },
                        agentGroups = remember(state.sessions, workspace) {
                            SessionListPresentation.agentGroups(state.sessions, workspace)
                        },
                        statuses = remember(state.sessions) {
                            SessionListPresentation.statusOptions(state.sessions)
                        },
                        onChange = { viewSettings = it },
                        onClose = { viewSettingsOpen = false },
                        modifier = Modifier,
                    )
                }

                val rows = view.sections.flatMap { it.sessions }
                if (rows.isEmpty()) {
                    Text(stringResource(R.string.sessions_empty))
                } else {
                    val projectCount = view.sections.count { it is SessionListSection.Project }
                    view.sections.forEachIndexed { index, section ->
                        if (section is SessionListSection.Project &&
                            view.sections.take(index).none { it is SessionListSection.Project }
                        ) {
                            ProjectTreeHeader(projectCount)
                        }
                        val sectionKey = sectionKey(section)
                        val collapsed = sectionKey in collapsedSectionKeys
                        SectionHeader(
                            section = section,
                            collapsed = collapsed,
                            onToggle = {
                                collapsedSectionKeys = if (collapsed) {
                                    collapsedSectionKeys - sectionKey
                                } else {
                                    collapsedSectionKeys + sectionKey
                                }
                            },
                        )
                        val batch = SessionListPresentation.batch(
                            sessions = section.sessions,
                            revealedSteps = revealedSectionKeys.count { it == sectionKey },
                        )
                        if (!collapsed) batch.visible.forEach { session ->
                            SessionRow(
                                title = session.title,
                                status = session.status,
                                updatedAt = session.updatedAt,
                                workspace = session.workspaceName
                                    ?: session.workspacePath.orEmpty(),
                                settings = viewSettings,
                                projectChild = section is SessionListSection.Project,
                                selected = session.id == state.selectedSessionId,
                                enabled = !state.busy,
                                onOpen = {
                                    onIntent(RemoteSessionIntent.Open(session.id))
                                    onOpen(session.id)
                                },
                                onActions = { actionsFor = session.id },
                            )
                            if (renaming == session.id) {
                                OutlinedTextField(
                                    value = renameDraft,
                                    onValueChange = { renameDraft = it },
                                    label = { Text(stringResource(R.string.session_rename_label)) },
                                    enabled = !state.busy,
                                    modifier = Modifier.fillMaxWidth(),
                                )
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    TextButton(
                                        onClick = {
                                            onIntent(
                                                RemoteSessionIntent.RenameSession(session.id, renameDraft),
                                            )
                                            renaming = null
                                        },
                                        enabled = !state.busy && renameDraft.isNotBlank(),
                                    ) { Text(stringResource(R.string.session_rename_confirm)) }
                                    TextButton(onClick = { renaming = null }) {
                                        Text(stringResource(R.string.pairing_dismiss))
                                    }
                                }
                            }
                        }
                        if (!collapsed && batch.nextCount > 0) {
                            TextButton(
                                onClick = {
                                    revealedSectionKeys = revealedSectionKeys + sectionKey
                                },
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .testTag(SESSION_SHOW_MORE_TEST_TAG_PREFIX + sectionKey),
                            ) {
                                Text(stringResource(R.string.sessions_show_more, batch.nextCount))
                            }
                        }
                    }
                    if (state.hasMore) {
                        TextButton(
                            onClick = { onIntent(RemoteSessionIntent.LoadMore) },
                            enabled = !state.busy,
                            modifier = Modifier.fillMaxWidth(),
                        ) { Text(stringResource(R.string.sessions_load_more)) }
                    }
                }
                Button(
                    onClick = { onIntent(RemoteSessionIntent.Refresh) },
                    enabled = !state.busy,
                ) { Text(stringResource(R.string.sessions_refresh)) }

                // Looked up by id rather than held as an object: a refresh that
                // lands while the sheet is open replaces every row, and holding
                // the old copy would show a title the list no longer has. If the
                // session is gone entirely the sheet closes with it.
                rows.firstOrNull { it.id == actionsFor }?.let { session ->
                    SessionActionSheet(
                        title = session.title,
                        status = session.status,
                        capabilities = SessionActionPolicy.resolve(
                            SessionActionScope.REMOTE,
                            session.agentType,
                            state.busy,
                        ),
                        onRename = {
                            renaming = session.id
                            renameDraft = session.title
                        },
                        onViewDetails = { detailsFor = session.id },
                        // Archive and export are local-storage operations, so
                        // the policy never offers them for a REMOTE scope and
                        // these cannot be reached from this list.
                        onArchive = {},
                        onExport = {},
                        onDelete = { onIntent(RemoteSessionIntent.DeleteSession(session.id)) },
                        onDismiss = { actionsFor = null },
                    )
                }
                rows.firstOrNull { it.id == detailsFor }?.let { session ->
                    SessionDetailsSheet(
                        title = session.title,
                        agentType = session.agentType,
                        status = session.status,
                        workspaceName = session.workspaceName,
                        workspacePath = session.workspacePath,
                        createdAt = session.createdAt,
                        updatedAt = session.updatedAt,
                        messageCount = session.messageCount,
                        onDismiss = { detailsFor = null },
                    )
                }
            }
        }
    }
}

/**
 * The selected workspace and the desktop's recents, as the grouping needs them.
 *
 * A workspace list we have not loaded yet is not an error here: the grouping
 * falls back to a single unnamed project, which is what the list looked like
 * before any of this existed.
 */
@Composable
private fun RemoteWorkspaceUiState.asSessionContext(): SessionWorkspaceContext {
    val ready = this as? RemoteWorkspaceUiState.Ready
    return remember(ready) {
        SessionWorkspaceContext(
            selectedPath = ready?.selected?.path.orEmpty(),
            selectedName = ready?.selected?.name.orEmpty(),
            selectedKind = ready?.selected?.kind.orEmpty(),
            recent = ready?.workspaces.orEmpty(),
        )
    }
}

/** One group heading; the project ones are named by the desktop, not by us. */
@Composable
private fun ProjectTreeHeader(projectCount: Int) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 44.dp)
            .testTag(SESSION_PROJECTS_TEST_TAG),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            stringResource(R.string.sessions_projects),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            projectCount.toString(),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** One collapsible group heading; project ones form the folder tree. */
@Composable
private fun SectionHeader(
    section: SessionListSection,
    collapsed: Boolean,
    onToggle: () -> Unit,
) {
    val label = when (section) {
        is SessionListSection.Chat -> stringResource(R.string.session_group_chat)
        is SessionListSection.Today -> stringResource(R.string.time_today)
        is SessionListSection.Yesterday -> stringResource(R.string.time_yesterday)
        is SessionListSection.Earlier -> stringResource(R.string.time_earlier)
        is SessionListSection.Project ->
            section.name.ifBlank { stringResource(R.string.view_settings_workspace) }
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 42.dp)
            .clip(RoundedCornerShape(8.dp))
            .clickable(onClick = onToggle)
            .padding(horizontal = if (section is SessionListSection.Project) 4.dp else 0.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (section is SessionListSection.Project) {
            Icon(
                painterResource(R.drawable.ic_symbol_folder),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.size(22.dp),
            )
        }
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            color = if (section is SessionListSection.Project) {
                MaterialTheme.colorScheme.onSurface
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        if (section is SessionListSection.Project) {
            Text(
                section.sessions.size.toString(),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Icon(
            painterResource(
                if (collapsed) R.drawable.ic_symbol_chevron_right
                else R.drawable.ic_symbol_chevron_down,
            ),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(16.dp),
        )
    }
}

private fun sectionKey(section: SessionListSection): String = when (section) {
    is SessionListSection.Chat -> "chat"
    is SessionListSection.Project -> "project:" + section.path
    is SessionListSection.Today -> "today"
    is SessionListSection.Yesterday -> "yesterday"
    is SessionListSection.Earlier -> "earlier"
}

/**
 * One session in the list, from `RemoteSessionList.ets#SessionRow`.
 *
 * A flat row rather than a filled button: the source paints a background only on
 * the session being read, so the list reads as a list with one thing marked in
 * it. Every row filled meant the marked one had nowhere left to go, and a column
 * of solid blocks buried the metadata line under its own contrast.
 *
 * Which parts of the metadata appear is the user's choice, and by default none
 * of them do — a row is its title until someone asks for more. Each part is
 * still conditional on having something to say: a timestamp the desktop did not
 * send is left out rather than shown as "unknown", and only `archived` has a
 * word of our own, so any other status is passed through as the desktop spelled
 * it. The row is shorter when it has only the title, as the source's height is.
 *
 * Long-pressing opens the same actions the overflow does, which is how the
 * source reaches them on a phone.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun SessionRow(
    title: String,
    status: String,
    updatedAt: String,
    workspace: String,
    settings: SessionViewSettings,
    projectChild: Boolean,
    selected: Boolean,
    enabled: Boolean,
    onOpen: () -> Unit,
    onActions: () -> Unit,
) {
    val now = remember(updatedAt) { System.currentTimeMillis() }
    val relative = remember(updatedAt, now) { SessionTimePresentation.relative(updatedAt, now) }
    val metadata = listOfNotNull(
        workspace.takeIf { settings.showWorkspace && it.isNotBlank() },
        relativeTimeText(relative).takeIf { settings.showUpdated },
        status.takeIf { settings.showStatus && it.isNotBlank() }?.let { statusText(it) },
    ).joinToString(" · ")

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = if (metadata.isEmpty()) 46.dp else 56.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(
                if (selected) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent,
            )
            .combinedClickable(
                enabled = enabled,
                onClick = onOpen,
                onLongClick = onActions,
            )
            .padding(start = if (projectChild) 28.dp else 10.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                title.ifBlank { stringResource(R.string.sidebar_untitled) },
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = if (selected) FontWeight.Medium else FontWeight.Normal,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (metadata.isNotEmpty()) {
                Text(
                    metadata,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        // The overflow keeps the destructive actions one deliberate tap away,
        // as `SessionMoreButton` does. Two permanent buttons under every row
        // made destroying a session as reachable as opening one.
        IconButton(onClick = onActions) {
            Icon(
                painterResource(R.drawable.ic_symbol_ellipsis),
                contentDescription = stringResource(R.string.session_actions),
                modifier = Modifier.size(18.dp),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** Null when the desktop sent nothing readable — see [SessionRowLabel]. */
@Composable
internal fun relativeTimeText(relative: RelativeTime): String? = when (relative) {
    RelativeTime.Unknown -> null
    RelativeTime.JustNow -> stringResource(R.string.time_just_now)
    is RelativeTime.MinutesAgo -> stringResource(R.string.time_minutes_ago, relative.minutes)
    is RelativeTime.HoursAgo -> stringResource(R.string.time_hours_ago, relative.hours)
    is RelativeTime.DaysAgo -> stringResource(R.string.time_days_ago, relative.days)
    // A plain ISO date rather than a localized one: it sits beside a status the
    // desktop wrote, and a sortable date reads the same in both languages.
    is RelativeTime.OnDate -> buildString {
        append(relative.year.toString().padStart(4, '0'))
        append('-')
        append(relative.month.toString().padStart(2, '0'))
        append('-')
        append(relative.day.toString().padStart(2, '0'))
    }
}

@Composable
private fun SessionFailure(state: RemoteSessionUiState.Failed, onRetry: () -> Unit) {
    Column {
        Text(
            stringResource(
                when (state.reason) {
                    RemoteSessionFailureReason.NO_WORKSPACE -> R.string.sessions_failed_no_workspace
                    RemoteSessionFailureReason.REMOTE_REJECTED -> R.string.sessions_failed_remote_rejected
                    RemoteSessionFailureReason.NETWORK -> R.string.sessions_failed_network
                    RemoteSessionFailureReason.TIMEOUT -> R.string.sessions_failed_timeout
                    RemoteSessionFailureReason.RATE_LIMITED -> R.string.sessions_failed_rate_limited
                    RemoteSessionFailureReason.PROTOCOL_MISMATCH -> R.string.sessions_failed_protocol_mismatch
                    RemoteSessionFailureReason.SESSION_NOT_FOUND -> R.string.sessions_failed_session_not_found
                    // Exhaustive on purpose rather than an `else`: every reason
                    // that reaches this screen was raised to say something
                    // specific, and a new one falling into the generic line is
                    // the bug this branch exists to prevent.
                    RemoteSessionFailureReason.TRANSPORT -> R.string.sessions_failed
                },
            ),
            color = MaterialTheme.colorScheme.error,
        )
        // The desktop wrote this sentence; it cannot be translated on this side,
        // so it sits under our heading as supporting detail.
        state.remoteMessage?.let { detail ->
            Text(
                detail,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        // Every reason above is retryable in the sense that matters here: the
        // request can be made again. Without this the screen is a dead end that
        // states a problem and offers nothing, which is how the account path
        // stranded a real session behind one bad decode.
        TextButton(onClick = onRetry) { Text(stringResource(R.string.sessions_retry)) }
    }
}
