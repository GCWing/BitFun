package com.openbitfun.mobile.app.ui.chat

import androidx.compose.foundation.clickable
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.app.ui.common.CircleControl
import com.openbitfun.mobile.app.ui.theme.generated.MobileDesignGeometry
import com.openbitfun.mobile.app.ui.theme.generated.MobileDesignTypography

internal const val CONVERSATION_TITLE_TEST_TAG: String = "conversation-title"
internal const val CONVERSATION_MENU_TEST_TAG: String = "conversation-menu"
internal const val CONVERSATION_RENAME_TEST_TAG: String = "conversation-rename"

/**
 * The bar above a transcript, ported from `pages/components/RemoteChatHeader.ets`.
 *
 * Two lines, centred: the session's own title over where it is running. The
 * second line is not decoration — the same list is reachable through a paired
 * desktop and through an account device, and only this line says which one is
 * answering.
 *
 * Renaming happens here rather than only in the session list because the title
 * a session deserves is usually obvious after reading it, not before. Tapping
 * the title opens the editor in place, as the source does.
 *
 * @param canStop whether a turn is running. The source's header popover is the
 * only place it offers to stop from, and this keeps that.
 */
@Composable
internal fun ConversationHeader(
    title: String,
    contextTitle: String,
    canStop: Boolean,
    enabled: Boolean,
    onBack: () -> Unit,
    onOpenSidebar: (() -> Unit)? = null,
    onRename: (String) -> Unit,
    onStop: () -> Unit,
    onShowUploadedFiles: () -> Unit = {},
    modifier: Modifier,
) {
    // Keyed on the title so a rename landing from the desktop closes the editor
    // rather than leaving the old text sitting in it.
    var editing by rememberSaveable(title) { mutableStateOf(false) }
    var draft by rememberSaveable(title) { mutableStateOf(title) }
    var menuOpen by rememberSaveable { mutableStateOf(false) }

    val hasSubtitle = contextTitle.isNotBlank()

    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(
                    if (hasSubtitle) MobileDesignGeometry.ConversationHeaderHeight
                    else MobileDesignGeometry.ConversationHeaderCompactHeight,
                )
                .padding(horizontal = MobileDesignGeometry.ContentGutter, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (onOpenSidebar != null) {
                CircleControl(
                    icon = R.drawable.ic_symbol_menu_lines,
                    glyphSize = 22,
                    contentDescription = stringResource(R.string.shell_open_sidebar),
                    onClick = onOpenSidebar,
                    modifier = Modifier.testTag(CONVERSATION_BACK_TEST_TAG),
                )
            } else {
                CircleControl(
                    icon = R.drawable.ic_symbol_chevron_left_wide,
                    glyphSize = 20,
                    glyphWidth = 15,
                    glyphHeight = 23,
                    contentDescription = stringResource(R.string.conversation_back),
                    onClick = onBack,
                    modifier = Modifier.testTag(CONVERSATION_BACK_TEST_TAG),
                )
            }

            Column(
                modifier = Modifier.weight(1f),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                Text(
                    title.ifBlank { stringResource(R.string.conversation_title_default) },
                    style = if (hasSubtitle) MobileDesignTypography.ConversationHeaderTitle
                    else MobileDesignTypography.TitleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    textAlign = TextAlign.Center,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier
                        .testTag(CONVERSATION_TITLE_TEST_TAG)
                        .clickable(enabled = enabled) {
                            // The title and actions are mutually exclusive surfaces.
                            menuOpen = false
                            // Opening the editor re-seeds the draft from the
                            // committed title; re-tapping the title while the
                            // editor is already open must not throw the user's
                            // in-progress text away.
                            if (!editing) draft = title
                            editing = true
                        },
                )
                if (hasSubtitle) {
                    Text(
                        contextTitle,
                        fontSize = 14.sp,
                        lineHeight = 18.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }

            Box {
                CircleControl(
                    icon = R.drawable.ic_symbol_ellipsis,
                    glyphSize = 20,
                    glyphWidth = 23,
                    glyphHeight = 7,
                    contentDescription = stringResource(R.string.session_actions),
                    onClick = {
                        // The editor and the menu are two answers to the same
                        // tap target area; opening one closes the other.
                        editing = false
                        menuOpen = !menuOpen
                    },
                    modifier = Modifier.testTag(CONVERSATION_MENU_TEST_TAG),
                )
                OpenBitFunHeaderActionMenu(
                    expanded = menuOpen,
                    onDismiss = { menuOpen = false },
                    sectionTitle = stringResource(R.string.session_section),
                    actions = buildList {
                        add(
                            OpenBitFunHeaderAction(
                                icon = R.drawable.ic_symbol_cloud,
                                label = stringResource(R.string.session_uploaded_files),
                                onClick = onShowUploadedFiles,
                            ),
                        )
                        if (canStop) {
                            add(
                                OpenBitFunHeaderAction(
                                    icon = R.drawable.ic_symbol_gearshape,
                                    label = stringResource(R.string.message_stop),
                                    onClick = onStop,
                                    dividerBefore = true,
                                ),
                            )
                        }
                    },
                )
            }
        }

        if (editing) {
            TitleEditor(
                draft = draft,
                enabled = enabled,
                onDraftChange = { draft = it },
                onSave = {
                    onRename(draft.trim())
                    editing = false
                },
                onCancel = { editing = false },
            )
        }
    }
}

/** Rename in place: a field and the two verdicts, as the source's row is. */
@Composable
internal fun TitleEditor(
    draft: String,
    enabled: Boolean,
    onDraftChange: (String) -> Unit,
    onSave: () -> Unit,
    onCancel: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(
                start = MobileDesignGeometry.ContentGutter,
                end = MobileDesignGeometry.ContentGutter,
                top = 10.dp,
                bottom = 8.dp,
            ),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val fieldShape = RoundedCornerShape(14.dp)
        BasicTextField(
            value = draft,
            onValueChange = onDraftChange,
            singleLine = true,
            enabled = enabled,
            textStyle = MaterialTheme.typography.bodyMedium.copy(
                fontSize = 14.sp,
                color = MaterialTheme.colorScheme.onSurface,
            ),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            decorationBox = { inner ->
                Box(contentAlignment = Alignment.CenterStart) {
                    if (draft.isEmpty()) {
                        Text(
                            stringResource(R.string.session_rename_label),
                            fontSize = 14.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    inner()
                }
            },
            modifier = Modifier
                .weight(1f)
                .height(42.dp)
                .clip(fieldShape)
                .background(MaterialTheme.colorScheme.surface)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, fieldShape)
                .padding(horizontal = 12.dp)
                .testTag(CONVERSATION_RENAME_TEST_TAG),
        )
        val saveEnabled = enabled && draft.isNotBlank()
        EditorButton(
            label = stringResource(R.string.session_rename_confirm),
            primary = true,
            enabled = saveEnabled,
            onClick = onSave,
        )
        EditorButton(
            label = stringResource(R.string.common_cancel),
            primary = false,
            enabled = true,
            onClick = onCancel,
        )
    }
}

@Composable
private fun EditorButton(
    label: String,
    primary: Boolean,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    val shape = RoundedCornerShape(14.dp)
    Box(
        modifier = Modifier
            .width(52.dp)
            .height(42.dp)
            .clip(shape)
            .background(
                if (primary && enabled) MaterialTheme.colorScheme.primary
                else MaterialTheme.colorScheme.surfaceVariant,
            )
            .clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            fontSize = 13.sp,
            fontWeight = FontWeight.Medium,
            color = if (primary && enabled) {
                MaterialTheme.colorScheme.onPrimary
            } else {
                MaterialTheme.colorScheme.onSurface
            },
            maxLines = 1,
        )
    }
}
