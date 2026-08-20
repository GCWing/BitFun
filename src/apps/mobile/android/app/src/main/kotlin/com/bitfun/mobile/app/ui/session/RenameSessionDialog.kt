package com.bitfun.mobile.app.ui.session

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import com.bitfun.mobile.app.R

internal const val RENAME_SESSION_TEST_TAG: String = "rename-session"

/**
 * Retitling one conversation.
 *
 * Owns the edited text itself so callers only have to say which session is being
 * renamed: every screen that offers this was otherwise carrying two pieces of
 * state — the flag and the buffer — that only ever changed together.
 *
 * The buffer survives a rotation but a blank one is refused, because clearing the
 * field is how a user backs out of a title they no longer want, not a request for
 * a nameless session.
 */
@Composable
internal fun RenameSessionDialog(
    currentTitle: String,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var text by rememberSaveable(currentTitle) { mutableStateOf(currentTitle) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.session_rename)) },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                label = { Text(stringResource(R.string.session_rename_label)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(
                enabled = text.isNotBlank(),
                onClick = {
                    onConfirm(text)
                    onDismiss()
                },
            ) { Text(stringResource(R.string.session_rename_confirm)) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.common_cancel)) }
        },
        modifier = Modifier.testTag(RENAME_SESSION_TEST_TAG),
    )
}
