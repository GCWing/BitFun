package com.openbitfun.mobile.app.ui.chat

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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.core.feature.session.MessageFileReference
import com.openbitfun.mobile.core.feature.session.MessageFileReferenceProjector
import com.openbitfun.mobile.core.feature.workspace.FilePreviewFormat
import com.openbitfun.mobile.core.feature.workspace.RemoteFileDownloadUiState

internal const val FILE_REFERENCE_CARD_TEST_TAG: String = "file-reference-card"
internal const val FILE_DOWNLOAD_ACTION_TEST_TAG: String = "file-download-action"

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
    download: RemoteFileDownloadUiState = RemoteFileDownloadUiState.None,
    onOpen: (String, String) -> Unit,
    onDownload: (String, String) -> Unit = { _, _ -> },
    downloadEnabled: Boolean = true,
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
            val downloadState = download.forReference(reference.reference, reference.remotePath)
            FileReferenceCard(
                reference = reference,
                selected = open,
                loading = open && previewLoading,
                onOpen = { onOpen(reference.reference, reference.label) },
                download = downloadState,
                onDownload = { onDownload(reference.reference, reference.label) },
                downloadEnabled = downloadEnabled,
            )
        }
    }
}

@Composable
private fun FileReferenceCard(
    reference: MessageFileReference,
    selected: Boolean,
    loading: Boolean,
    download: RemoteFileDownloadUiState,
    onOpen: () -> Unit,
    onDownload: () -> Unit,
    downloadEnabled: Boolean,
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
            modifier = Modifier.padding(12.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Row(
                modifier = Modifier.weight(1f).height(44.dp).clickable(onClick = onOpen),
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
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Medium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        download.statusText(),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            val actionEnabled = downloadEnabled &&
                download !is RemoteFileDownloadUiState.Loading &&
                download !is RemoteFileDownloadUiState.AwaitingSave
            Box(
                modifier = Modifier.size(44.dp).testTag(FILE_DOWNLOAD_ACTION_TEST_TAG)
                    .alpha(if (actionEnabled) 1f else 0.55f)
                    .clip(RoundedCornerShape(12.dp)).clickable(
                    enabled = actionEnabled,
                    onClick = onDownload,
                ),
                contentAlignment = Alignment.Center,
            ) {
                if (download is RemoteFileDownloadUiState.Loading ||
                    download is RemoteFileDownloadUiState.AwaitingSave
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(18.dp), strokeWidth = 2.dp)
                } else {
                    Icon(
                        painterResource(R.drawable.ic_symbol_arrow_down_to_line),
                        contentDescription = stringResource(R.string.file_download),
                        modifier = Modifier.size(19.dp),
                    )
                }
            }
        }
    }
}

/** Also read by the file preview pane's status band, which says it the same way. */
@Composable
internal fun RemoteFileDownloadUiState.statusText(): String = when (this) {
    RemoteFileDownloadUiState.None -> stringResource(R.string.file_reference_desktop)
    is RemoteFileDownloadUiState.Loading -> if (totalBytes > 0) {
        stringResource(
            R.string.file_downloading_progress,
            FilePreviewFormat.bytes(downloadedBytes),
            FilePreviewFormat.bytes(totalBytes),
        )
    } else {
        stringResource(R.string.file_downloading)
    }
    is RemoteFileDownloadUiState.AwaitingSave -> stringResource(R.string.file_saving)
    is RemoteFileDownloadUiState.Saved -> stringResource(R.string.common_done)
    is RemoteFileDownloadUiState.Failed -> stringResource(R.string.file_download_failed)
}

internal fun RemoteFileDownloadUiState.forReference(
    reference: String,
    remotePath: String,
): RemoteFileDownloadUiState {
    val target = when (this) {
        RemoteFileDownloadUiState.None -> return this
        is RemoteFileDownloadUiState.Loading -> target
        is RemoteFileDownloadUiState.AwaitingSave -> target
        is RemoteFileDownloadUiState.Saved -> target
        is RemoteFileDownloadUiState.Failed -> target
    }
    return if (target.path == reference || target.remotePath == remotePath) this else RemoteFileDownloadUiState.None
}
