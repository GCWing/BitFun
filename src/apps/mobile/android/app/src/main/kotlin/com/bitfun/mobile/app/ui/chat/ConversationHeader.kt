package com.bitfun.mobile.app.ui.chat

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.common.CircleControl

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
    onRename: (String) -> Unit,
    onOpenSettings: () -> Unit,
    onStop: () -> Unit,
    modifier: Modifier,
) {
    // Keyed on the title so a rename landing from the desktop closes the editor
    // rather than leaving the old text sitting in it.
    var editing by rememberSaveable(title) { mutableStateOf(false) }
    var draft by rememberSaveable(title) { mutableStateOf(title) }
    var menuOpen by rememberSaveable { mutableStateOf(false) }

    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 18.dp, end = 18.dp, top = 14.dp, bottom = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            CircleControl(
                icon = R.drawable.ic_symbol_chevron_left,
                glyphSize = 20,
                contentDescription = stringResource(R.string.conversation_back),
                onClick = onBack,
                modifier = Modifier.testTag(CONVERSATION_BACK_TEST_TAG),
            )

            Column(
                modifier = Modifier.weight(1f),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                Text(
                    title.ifBlank { stringResource(R.string.conversation_title_default) },
                    style = MaterialTheme.typography.titleMedium,
                    textAlign = TextAlign.Center,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier
                        .testTag(CONVERSATION_TITLE_TEST_TAG)
                        .clickable(enabled = enabled) {
                            draft = title
                            editing = !editing
                        },
                )
                Text(
                    contextTitle,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            Box {
                CircleControl(
                    icon = R.drawable.ic_symbol_ellipsis,
                    glyphSize = 20,
                    contentDescription = stringResource(R.string.session_actions),
                    onClick = {
                        // The editor and the menu are two answers to the same
                        // tap target area; opening one closes the other.
                        editing = false
                        menuOpen = true
                    },
                    modifier = Modifier.testTag(CONVERSATION_MENU_TEST_TAG),
                )
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.conversation_settings)) },
                        onClick = {
                            menuOpen = false
                            onOpenSettings()
                        },
                    )
                    // Only while something is running: an item that does nothing
                    // is worse than an item that is not there.
                    if (canStop) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.message_stop)) },
                            onClick = {
                                menuOpen = false
                                onStop()
                            },
                        )
                    }
                }
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
private fun TitleEditor(
    draft: String,
    enabled: Boolean,
    onDraftChange: (String) -> Unit,
    onSave: () -> Unit,
    onCancel: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 18.dp, end = 18.dp, top = 10.dp, bottom = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OutlinedTextField(
            value = draft,
            onValueChange = onDraftChange,
            label = { Text(stringResource(R.string.session_rename_label)) },
            // A title is one line by definition, and the return key is worth
            // more as "done" than as a newline the relay would strip anyway.
            singleLine = true,
            enabled = enabled,
            shape = RoundedCornerShape(14.dp),
            modifier = Modifier.weight(1f).testTag(CONVERSATION_RENAME_TEST_TAG),
        )
        Button(
            onClick = onSave,
            enabled = enabled && draft.isNotBlank(),
            shape = RoundedCornerShape(14.dp),
            modifier = Modifier.height(48.dp),
        ) { Text(stringResource(R.string.session_rename_confirm)) }
        TextButton(onClick = onCancel, modifier = Modifier.height(48.dp)) {
            Text(stringResource(R.string.common_cancel))
        }
    }
}
