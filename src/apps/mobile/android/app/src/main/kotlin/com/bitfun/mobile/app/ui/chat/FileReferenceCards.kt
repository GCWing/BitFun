package com.bitfun.mobile.app.ui.chat

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.core.feature.session.MessageFileReference
import com.bitfun.mobile.core.feature.session.MessageFileReferenceProjector

internal const val FILE_REFERENCE_CARD_TEST_TAG: String = "file-reference-card"

/**
 * The files an agent turn named, as cards under the turn — ported from
 * `MessageFileCards` in `pages/components/ChatMessageContent.ets`.
 *
 * The same paths are already tappable inside the prose. These exist because on a
 * phone a link inside a justified paragraph is a small target next to other
 * small targets, and because the projection dedupes: a turn that mentions one
 * file four times gets one card, not four links to hunt through.
 *
 * The source pairs each card with a download button. There is no download here,
 * and the button is left out rather than drawn dead: `RemoteWorkspaceIntent` has
 * no such intent and the desktop has no command behind it, so the whole path is
 * missing rather than merely unwired on this client.
 */
@Composable
internal fun FileReferenceCards(
    text: String,
    previewingRemotePath: String,
    previewLoading: Boolean,
    onOpen: (String, String) -> Unit,
    modifier: Modifier,
) {
    // Projecting is a markdown parse; a streaming turn recomposes on every
    // chunk, so it is keyed on the text rather than run each time.
    val references = remember(text) { MessageFileReferenceProjector.project(text) }
    if (references.isEmpty()) return

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        references.forEach { reference ->
            val open = reference.remotePath == previewingRemotePath
            FileReferenceCard(
                reference = reference,
                selected = open,
                loading = open && previewLoading,
                onOpen = { onOpen(reference.reference, reference.label) },
            )
        }
    }
}

@Composable
private fun FileReferenceCard(
    reference: MessageFileReference,
    selected: Boolean,
    loading: Boolean,
    onOpen: () -> Unit,
) {
    Surface(
        shape = RoundedCornerShape(14.dp),
        color = if (selected) {
            MaterialTheme.colorScheme.secondaryContainer
        } else {
            MaterialTheme.colorScheme.surface
        },
        border = BorderStroke(
            width = 1.dp,
            color = if (selected) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.outlineVariant
            },
        ),
        modifier = Modifier.fillMaxWidth().testTag(FILE_REFERENCE_CARD_TEST_TAG),
    ) {
        Row(
            modifier = Modifier.clickable(onClick = onOpen).padding(12.dp).height(44.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                modifier = Modifier
                    .size(34.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant),
                contentAlignment = Alignment.Center,
            ) {
                if (loading) {
                    CircularProgressIndicator(modifier = Modifier.size(17.dp), strokeWidth = 2.dp)
                } else {
                    Icon(
                        painterResource(R.drawable.ic_symbol_doc_text),
                        contentDescription = null,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                Text(
                    reference.label,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                // The second line says where the file is, not what it is called
                // again — every card here points at the paired desktop.
                Text(
                    stringResource(R.string.file_reference_desktop),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
