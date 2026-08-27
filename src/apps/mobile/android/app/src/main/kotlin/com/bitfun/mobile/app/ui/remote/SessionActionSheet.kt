package com.bitfun.mobile.app.ui.remote

import androidx.annotation.DrawableRes
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
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
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.settings.statusText
import com.bitfun.mobile.app.ui.common.AdaptiveModalSurface
import com.bitfun.mobile.core.feature.session.SessionActionCapabilities
import com.bitfun.mobile.core.feature.layout.SettingsPlacement
import com.bitfun.mobile.core.feature.session.SessionTimePresentation
import com.bitfun.mobile.app.ui.theme.generated.MobileDesignGeometry

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
    onViewDetails: () -> Unit,
    onArchive: (() -> Unit)? = null,
    onExport: (() -> Unit)? = null,
    onDelete: () -> Unit,
    onDismiss: () -> Unit,
) {
    // Deliberately not `rememberSaveable`: the confirmation is a step inside a
    // sheet that is itself gone after a process death, and restoring "about to
    // delete" without the tap that got there is not a state worth rebuilding.
    var confirmingDelete by remember { mutableStateOf(false) }
    // Archive and export are local-storage operations a remote session can never
    // offer, so those callbacks are optional: a surface without them renders no
    // row instead of rendering one that does nothing.
    val showArchive = capabilities.canArchive && onArchive != null
    val showExport = capabilities.canExport && onExport != null

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = RoundedCornerShape(
            topStart = MobileDesignGeometry.PopoverRadius,
            topEnd = MobileDesignGeometry.PopoverRadius,
        ),
        containerColor = MaterialTheme.colorScheme.surface,
        tonalElevation = 0.dp,
        dragHandle = null,
        modifier = Modifier
            .testTag(SESSION_ACTIONS_TEST_TAG)
            .border(
                width = 1.dp,
                color = MaterialTheme.colorScheme.outlineVariant,
                shape = RoundedCornerShape(
                    topStart = MobileDesignGeometry.PopoverRadius,
                    topEnd = MobileDesignGeometry.PopoverRadius,
                ),
            ),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 16.dp, end = 16.dp, top = 10.dp, bottom = 18.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Box(
                modifier = Modifier
                    .padding(bottom = 10.dp)
                    .size(width = 36.dp, height = 4.dp)
                    .clip(RoundedCornerShape(2.dp))
                    .background(MaterialTheme.colorScheme.outlineVariant),
            )
            Row(
                modifier = Modifier.fillMaxWidth().height(52.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(
                    modifier = Modifier.weight(1f),
                    verticalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    Text(
                        stringResource(R.string.session_actions),
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Medium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        title.ifBlank { stringResource(R.string.sidebar_untitled) },
                        fontSize = 15.sp,
                        fontWeight = FontWeight.Medium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Box(
                    modifier = Modifier
                        .size(40.dp)
                        .clip(RoundedCornerShape(20.dp))
                        .clickable(onClick = onDismiss),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ic_symbol_xmark),
                        contentDescription = stringResource(R.string.common_close),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }

            HorizontalDivider(modifier = Modifier.padding(top = 6.dp, bottom = 8.dp))

            if (confirmingDelete) {
                Text(
                    stringResource(R.string.session_delete_confirm),
                    fontSize = 13.sp,
                    lineHeight = 19.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                )
                Row(
                    modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    ConfirmationButton(
                        label = stringResource(R.string.common_cancel),
                        containerColor = MaterialTheme.colorScheme.surfaceVariant,
                        contentColor = MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier.weight(1f),
                        onClick = { confirmingDelete = false },
                    )
                    ConfirmationButton(
                        label = stringResource(R.string.session_delete),
                        containerColor = MaterialTheme.colorScheme.error,
                        contentColor = MaterialTheme.colorScheme.onError,
                        modifier = Modifier.weight(1f),
                        onClick = {
                            onDelete()
                            onDismiss()
                        },
                    )
                }
                return@Column
            }

            if (capabilities.canViewDetails) {
                ActionRow(R.drawable.ic_symbol_info_circle, stringResource(R.string.session_view_details)) {
                    onViewDetails()
                    onDismiss()
                }
            }
            if (showArchive) {
                val archived = status.equals("archived", ignoreCase = true)
                val label = stringResource(
                    if (archived) R.string.session_unarchive else R.string.session_archive,
                )
                ActionRow(R.drawable.ic_symbol_archivebox, label) {
                    onArchive()
                    onDismiss()
                }
            }
            if (showExport) {
                // The label says "Copy Markdown" for Harmony parity, but Android
                // opens an Intent.ACTION_SEND text/plain share sheet; this icon
                // must not promise a file or cloud export.
                ActionRow(
                    R.drawable.ic_symbol_doc_text_badge_arrow_up,
                    stringResource(R.string.general_chat_export),
                ) {
                    onExport()
                    onDismiss()
                }
            }
            if (capabilities.canDelete) {
                if (showArchive || showExport) {
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

/** Wide-sidebar counterpart of [SessionActionSheet]. It renders the same rows
 * next to the row that opened it and keeps the native popup lifecycle arrowless. */
@Composable
internal fun SessionActionPopup(
    anchorBounds: IntRect,
    title: String,
    status: String,
    capabilities: SessionActionCapabilities,
    onViewDetails: () -> Unit,
    onArchive: (() -> Unit)? = null,
    onExport: (() -> Unit)? = null,
    onDelete: () -> Unit,
    onDismiss: () -> Unit,
) {
    var confirmingDelete by remember { mutableStateOf(false) }
    val showArchive = capabilities.canArchive && onArchive != null
    val showExport = capabilities.canExport && onExport != null
    val targetBounds = anchorBounds
    val positionProvider = remember(targetBounds) {
        object : PopupPositionProvider {
            override fun calculatePosition(
                anchorBounds: IntRect,
                windowSize: IntSize,
                layoutDirection: LayoutDirection,
                popupContentSize: IntSize,
            ): IntOffset {
                val desiredX = targetBounds.right + 6
                val desiredY = (
                    targetBounds.top + targetBounds.bottom -
                        popupContentSize.height
                    ) / 2
                return IntOffset(
                    x = desiredX.coerceIn(
                        8,
                        (windowSize.width - popupContentSize.width - 8).coerceAtLeast(8),
                    ),
                    y = desiredY.coerceIn(
                        8,
                        (windowSize.height - popupContentSize.height - 8).coerceAtLeast(8),
                    ),
                )
            }
        }
    }
    Popup(
        popupPositionProvider = positionProvider,
        onDismissRequest = onDismiss,
        properties = PopupProperties(focusable = true),
    ) {
        Column(
            modifier = Modifier
                .width(300.dp)
                .shadow(20.dp, RoundedCornerShape(MobileDesignGeometry.PopoverRadius))
                .clip(RoundedCornerShape(MobileDesignGeometry.PopoverRadius))
                .background(MaterialTheme.colorScheme.surface)
                .border(
                    1.dp,
                    MaterialTheme.colorScheme.outlineVariant,
                    RoundedCornerShape(MobileDesignGeometry.PopoverRadius),
                )
                .padding(start = 16.dp, end = 16.dp, top = 10.dp, bottom = 18.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth().height(52.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    Text(
                        stringResource(R.string.session_actions),
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Medium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        title.ifBlank { stringResource(R.string.sidebar_untitled) },
                        fontSize = 15.sp,
                        fontWeight = FontWeight.Medium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Box(
                    modifier = Modifier.size(40.dp).clip(RoundedCornerShape(20.dp)).clickable(onClick = onDismiss),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        painterResource(R.drawable.ic_symbol_xmark),
                        contentDescription = stringResource(R.string.common_close),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }
            HorizontalDivider(modifier = Modifier.padding(top = 6.dp, bottom = 8.dp))
            if (confirmingDelete) {
                Text(
                    stringResource(R.string.session_delete_confirm),
                    fontSize = 13.sp,
                    lineHeight = 19.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                )
                Row(
                    modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    ConfirmationButton(
                        stringResource(R.string.common_cancel),
                        MaterialTheme.colorScheme.surfaceVariant,
                        MaterialTheme.colorScheme.onSurface,
                        Modifier.weight(1f),
                    ) { confirmingDelete = false }
                    ConfirmationButton(
                        stringResource(R.string.session_delete),
                        MaterialTheme.colorScheme.error,
                        MaterialTheme.colorScheme.onError,
                        Modifier.weight(1f),
                    ) {
                        onDelete()
                        onDismiss()
                    }
                }
            } else {
                if (capabilities.canViewDetails) {
                    ActionRow(R.drawable.ic_symbol_info_circle, stringResource(R.string.session_view_details)) {
                        onViewDetails(); onDismiss()
                    }
                }
                if (showArchive) {
                    ActionRow(
                        R.drawable.ic_symbol_archivebox,
                        stringResource(
                            if (status.equals("archived", true)) R.string.session_unarchive
                            else R.string.session_archive,
                        ),
                    ) { onArchive(); onDismiss() }
                }
                if (showExport) {
                    ActionRow(
                        R.drawable.ic_symbol_doc_text_badge_arrow_up,
                        stringResource(R.string.general_chat_export),
                    ) {
                        onExport(); onDismiss()
                    }
                }
                if (capabilities.canDelete) {
                    if (showArchive || showExport) {
                        HorizontalDivider(modifier = Modifier.padding(vertical = 6.dp))
                    }
                    ActionRow(
                        R.drawable.ic_symbol_trash,
                        stringResource(R.string.session_delete),
                        destructive = true,
                    ) { confirmingDelete = true }
                }
            }
        }
    }
}

@Composable
private fun ConfirmationButton(
    label: String,
    containerColor: androidx.compose.ui.graphics.Color,
    contentColor: androidx.compose.ui.graphics.Color,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    Box(
        modifier = modifier
            .height(44.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(containerColor)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = contentColor,
            fontSize = 14.sp,
            fontWeight = if (containerColor == MaterialTheme.colorScheme.error) {
                FontWeight.Medium
            } else {
                FontWeight.Normal
            },
            maxLines = 1,
        )
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
            fontSize = 15.sp,
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
    placement: SettingsPlacement,
    onDismiss: () -> Unit,
) {
    val now = remember(createdAt, updatedAt) { System.currentTimeMillis() }
    val created = relativeTimeText(
        remember(createdAt, now) { SessionTimePresentation.relative(createdAt, now) },
    )
    val updated = relativeTimeText(
        remember(updatedAt, now) { SessionTimePresentation.relative(updatedAt, now) },
    )

    AdaptiveModalSurface(
        visible = true,
        placement = placement,
        onDismissRequest = onDismiss,
    ) { surfaceModifier ->
        Column(
            modifier = surfaceModifier
                .testTag(SESSION_DETAILS_TEST_TAG)
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
