package com.bitfun.mobile.app.ui.remote

import androidx.annotation.DrawableRes
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.settings.statusText
import com.bitfun.mobile.core.feature.session.SessionActionCapabilities
import com.bitfun.mobile.core.feature.session.SessionTimePresentation

internal const val SESSION_ACTIONS_TEST_TAG: String = "session-actions"
internal const val SESSION_DETAILS_TEST_TAG: String = "session-details"

/**
 * A session row's overflow menu, ported from
 * `pages/components/SessionActionSurface.ets`.
 *
 * Which rows appear is [SessionActionCapabilities]' answer, not this file's:
 * archive and export are local-storage operations that a remote session cannot
 * offer however it is typed, and while a command is in flight nothing is
 * offered at all because the row this was opened over may not survive the
 * answer.
 *
 * Deleting replaces the action list in place instead of stacking a dialog on
 * top of it, as the source does. Two layers of scrim over a bottom sheet gives
 * the confirmation two dismiss gestures with different meanings — tapping
 * outside would cancel the delete or the whole menu depending on how far the
 * finger landed.
 *
 * Takes the fields rather than the session: `RemoteSession` lives in
 * `core-domain`, which the app is not allowed to import (design doc §5 rule 3),
 * and re-exporting a domain type through `core-feature` just to name it here
 * would defeat the rule rather than satisfy it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SessionActionSheet(
    title: String,
    status: String,
    capabilities: SessionActionCapabilities,
    onRename: () -> Unit,
    onViewDetails: () -> Unit,
    onArchive: () -> Unit,
    onExport: () -> Unit,
    onDelete: () -> Unit,
    onDismiss: () -> Unit,
) {
    // Deliberately not `rememberSaveable`: the confirmation is a step inside a
    // sheet that is itself gone after a process death, and restoring "about to
    // delete" without the tap that got there is not a state worth rebuilding.
    var confirmingDelete by remember { mutableStateOf(false) }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(),
        modifier = Modifier.testTag(SESSION_ACTIONS_TEST_TAG),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 16.dp, end = 16.dp, bottom = 24.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        stringResource(R.string.session_actions),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        title.ifBlank { stringResource(R.string.sidebar_untitled) },
                        style = MaterialTheme.typography.titleSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.common_close)) }
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))

            if (confirmingDelete) {
                Text(
                    stringResource(R.string.session_delete_confirm),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Row(
                    modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    FilledTonalButton(
                        onClick = { confirmingDelete = false },
                        modifier = Modifier.weight(1f),
                    ) { Text(stringResource(R.string.common_cancel)) }
                    Button(
                        onClick = {
                            onDelete()
                            onDismiss()
                        },
                        colors = ButtonDefaults.buttonColors(
                            containerColor = MaterialTheme.colorScheme.error,
                            contentColor = MaterialTheme.colorScheme.onError,
                        ),
                        modifier = Modifier.weight(1f),
                    ) { Text(stringResource(R.string.session_delete)) }
                }
                return@Column
            }

            if (capabilities.canViewDetails) {
                ActionRow(R.drawable.ic_symbol_info_circle, stringResource(R.string.session_view_details)) {
                    onViewDetails()
                    onDismiss()
                }
            }
            // Renaming has no counterpart in the source's surface — HarmonyOS
            // has no rename at all — but the intent and the desktop command
            // both exist and Android already shipped it. Keeping it here rather
            // than as a second naked button beside the row is the alignment;
            // dropping a working action to match an absence would not be.
            ActionRow(R.drawable.ic_symbol_square_and_pencil, stringResource(R.string.session_rename)) {
                onRename()
                onDismiss()
            }
            if (capabilities.canArchive) {
                val archived = status.equals("archived", ignoreCase = true)
                val label = stringResource(
                    if (archived) R.string.session_unarchive else R.string.session_archive,
                )
                ActionRow(R.drawable.ic_symbol_archivebox, label) {
                    onArchive()
                    onDismiss()
                }
            }
            if (capabilities.canExport) {
                // The source draws a cloud here; the label says copy, and on
                // Android the action puts Markdown on the clipboard.
                ActionRow(R.drawable.ic_symbol_cloud, stringResource(R.string.general_chat_export)) {
                    onExport()
                    onDismiss()
                }
            }
            if (capabilities.canDelete) {
                if (capabilities.canArchive || capabilities.canExport) {
                    HorizontalDivider(modifier = Modifier.padding(vertical = 6.dp))
                }
                ActionRow(
                    icon = R.drawable.ic_symbol_trash,
                    label = stringResource(R.string.session_delete),
                    destructive = true,
                ) { confirmingDelete = true }
            }
        }
    }
}

@Composable
private fun ActionRow(
    @DrawableRes icon: Int,
    label: String,
    destructive: Boolean,
    onClick: () -> Unit,
) {
    val tint = if (destructive) {
        MaterialTheme.colorScheme.error
    } else {
        MaterialTheme.colorScheme.onSurfaceVariant
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .clickable(onClick = onClick)
            .heightIn(min = 46.dp)
            .padding(horizontal = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painterResource(icon),
            contentDescription = null,
            tint = tint,
            // The source draws these at fontSize 19, a size up from a list
            // glyph: this sheet is where a session gets destroyed.
            modifier = Modifier.size(19.dp),
        )
        Text(
            label,
            style = MaterialTheme.typography.bodyLarge,
            color = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun ActionRow(@DrawableRes icon: Int, label: String, onClick: () -> Unit) {
    ActionRow(icon, label, destructive = false, onClick = onClick)
}

/**
 * The read-only facts about a session, ported from
 * `pages/components/SessionDetailsView.ets`.
 *
 * Every row is conditional on having something to say. A timestamp the desktop
 * never sent is left out rather than rendered as "unknown", which is the same
 * rule the list rows follow.
 *
 * Takes fields rather than the session for the reason [SessionActionSheet] does.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SessionDetailsSheet(
    title: String,
    agentType: String,
    status: String,
    workspaceName: String?,
    workspacePath: String?,
    createdAt: String,
    updatedAt: String,
    messageCount: Int,
    onDismiss: () -> Unit,
) {
    val now = remember(createdAt, updatedAt) { System.currentTimeMillis() }
    val created = relativeTimeText(
        remember(createdAt, now) { SessionTimePresentation.relative(createdAt, now) },
    )
    val updated = relativeTimeText(
        remember(updatedAt, now) { SessionTimePresentation.relative(updatedAt, now) },
    )

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(),
        modifier = Modifier.testTag(SESSION_DETAILS_TEST_TAG),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 20.dp, end = 16.dp, bottom = 24.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        stringResource(R.string.session_details),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        title.ifBlank { stringResource(R.string.sidebar_untitled) },
                        style = MaterialTheme.typography.titleMedium,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.common_close)) }
            }

            HorizontalDivider(modifier = Modifier.padding(top = 12.dp))

            Column(modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
                DetailRow(
                    stringResource(R.string.session_agent_type),
                    agentType.ifBlank { stringResource(R.string.common_unknown) },
                )
                workspaceName?.takeIf { it.isNotBlank() }?.let {
                    DetailRow(stringResource(R.string.session_workspace), it)
                }
                workspacePath?.takeIf { it.isNotBlank() }?.let {
                    PathRow(stringResource(R.string.session_workspace_path), it)
                }
                created?.let { DetailRow(stringResource(R.string.session_created_at), it) }
                updated?.let { DetailRow(stringResource(R.string.session_updated_at), it) }
                DetailRow(
                    stringResource(R.string.session_message_count),
                    messageCount.coerceAtLeast(0).toString(),
                )
                if (status.isNotBlank()) {
                    DetailRow(stringResource(R.string.session_status), statusText(status))
                }
            }
        }
    }
}

@Composable
private fun DetailRow(label: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.width(104.dp),
        )
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.End,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
    }
    HorizontalDivider()
}

/**
 * A path gets its own full-width row and stays selectable.
 *
 * The source marks it `CopyOptions.LocalDevice` for the same reason: it is the
 * one value here a user needs somewhere else, and it is too long to retype.
 */
@Composable
private fun PathRow(label: String, value: String) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        SelectionContainer {
            Text(
                value,
                style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(6.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant)
                    .padding(horizontal = 10.dp, vertical = 8.dp),
            )
        }
    }
    HorizontalDivider()
}
