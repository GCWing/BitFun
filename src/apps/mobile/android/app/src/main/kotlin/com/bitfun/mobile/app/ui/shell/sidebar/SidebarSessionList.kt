package com.bitfun.mobile.app.ui.shell.sidebar

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.unit.sp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.core.feature.shell.SidebarSections
import com.bitfun.mobile.core.feature.shell.SidebarSessionRow

internal const val SIDEBAR_PINNED_TEST_TAG: String = "app-sidebar-pinned"
internal const val SIDEBAR_ARCHIVED_TEST_TAG: String = "app-sidebar-archived"
internal const val SIDEBAR_MORE_TEST_TAG: String = "app-sidebar-more"

/**
 * The conversation list, ported from `LocalSessionContent` in `AppSidebar.ets`.
 *
 * Three sections rather than one list: what is held above, what is recent, and
 * what has been filed away behind a disclosure that only says how much is in
 * there. The archive stays collapsed because its whole point is to be out of the
 * way of the list a user actually reads.
 */
@Composable
internal fun SidebarSessionList(
    sections: SidebarSections,
    selectedSessionId: String?,
    activeActionSessionId: String?,
    searching: Boolean,
    archivedExpanded: Boolean,
    onToggleArchived: () -> Unit,
    onOpenSession: (SidebarSessionRow) -> Unit,
    onOpenActions: (SidebarSessionRow) -> Unit,
    workspaceContent: @Composable () -> Unit,
    modifier: Modifier,
) {
    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        if (sections.pinned.isNotEmpty()) {
            item(key = "pinned-header") {
                SectionLabel(
                    stringResource(R.string.sidebar_pinned),
                    Modifier.testTag(SIDEBAR_PINNED_TEST_TAG),
                )
            }
            items(sections.pinned, key = { "pinned-" + it.id }) { session ->
                SessionRow(
                    session = session,
                    pinned = true,
                    selected = session.id == selectedSessionId ||
                        session.id == activeActionSessionId,
                    onOpen = onOpenSession,
                    onOpenActions = onOpenActions,
                )
            }
        }

        item(key = "recent-header") { SectionLabel(stringResource(R.string.sidebar_recent), Modifier) }

        if (sections.recent.isEmpty()) {
            item(key = "recent-empty") {
                Text(
                    stringResource(
                        if (searching) R.string.sidebar_no_search_result else R.string.sidebar_recent_empty,
                    ),
                    fontSize = 14.sp,
                    fontWeight = FontWeight.Medium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
                )
            }
        } else {
            items(sections.recent, key = { "recent-" + it.id }) { session ->
                SessionRow(
                    session = session,
                    pinned = false,
                    selected = session.id == selectedSessionId ||
                        session.id == activeActionSessionId,
                    onOpen = onOpenSession,
                    onOpenActions = onOpenActions,
                )
            }
        }

        if (sections.archivedCount > 0) {
            item(key = "archived-disclosure") {
                ArchivedDisclosureRow(
                    count = sections.archivedCount,
                    expanded = archivedExpanded,
                    onToggle = onToggleArchived,
                )
            }
            if (archivedExpanded) {
                items(sections.archived, key = { "archived-" + it.id }) { session ->
                    SessionRow(
                        session = session,
                        pinned = false,
                        selected = session.id == selectedSessionId ||
                            session.id == activeActionSessionId,
                        onOpen = onOpenSession,
                        onOpenActions = onOpenActions,
                    )
                }
            }
        }

        item(key = "workspace-section") { workspaceContent() }

        // Room for the footer, which floats over the bottom of this list rather
        // than pushing it up — the same 84dp the source reserves.
        item(key = "footer-room") { Box(Modifier.height(84.dp)) }
    }
}

@Composable
private fun SectionLabel(text: String, modifier: Modifier) {
    Text(
        text,
        fontSize = 14.sp,
        fontWeight = FontWeight.Medium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.fillMaxWidth().padding(top = 16.dp, bottom = 6.dp),
    )
}

/**
 * One conversation.
 *
 * Long-press opens the same menu as the overflow button: on a list this narrow
 * the button is a small target, and holding a row is what a phone user reaches
 * for first.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun SessionRow(
    session: SidebarSessionRow,
    pinned: Boolean,
    selected: Boolean,
    onOpen: (SidebarSessionRow) -> Unit,
    onOpenActions: (SidebarSessionRow) -> Unit,
) {
    val title = session.title.ifBlank { stringResource(R.string.sidebar_untitled) }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(44.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(if (selected) MaterialTheme.colorScheme.surfaceVariant else Color.Transparent)
            .combinedClickable(
                onClick = { onOpen(session) },
                onLongClick = { onOpenActions(session) },
            )
            .padding(start = 12.dp, end = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (pinned) {
            Icon(
                painterResource(R.drawable.ic_symbol_checkmark_circle),
                contentDescription = stringResource(R.string.sidebar_pinned),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(18.dp),
            )
        }
        Text(
            title,
            fontSize = 15.sp,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        Box(
            modifier = Modifier
                .width(34.dp)
                .height(40.dp)
                .clip(RoundedCornerShape(8.dp))
                .clickable { onOpenActions(session) }
                .testTag(SIDEBAR_MORE_TEST_TAG),
            contentAlignment = Alignment.Center,
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

/** The archive, shown as how much is in it rather than as what is in it. */
@Composable
private fun ArchivedDisclosureRow(count: Int, expanded: Boolean, onToggle: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 8.dp)
            .height(46.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(if (expanded) MaterialTheme.colorScheme.surfaceVariant else Color.Transparent)
            .clickable(onClick = onToggle)
            .padding(start = 12.dp, end = 12.dp)
            .testTag(SIDEBAR_ARCHIVED_TEST_TAG),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painterResource(R.drawable.ic_symbol_archivebox),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(20.dp),
        )
        Text(
            stringResource(R.string.sidebar_archived),
            fontSize = 14.sp,
            fontWeight = FontWeight.Medium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
        Box(
            modifier = Modifier
                .width(24.dp)
                .height(22.dp)
                .clip(RoundedCornerShape(11.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                count.toString(),
                fontSize = 12.sp,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Icon(
            painterResource(
                if (expanded) {
                    R.drawable.ic_symbol_chevron_up
                } else {
                    R.drawable.ic_symbol_chevron_down
                },
            ),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(16.dp),
        )
    }
}
