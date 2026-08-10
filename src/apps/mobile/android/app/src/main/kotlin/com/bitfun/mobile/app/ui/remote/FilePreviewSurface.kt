package com.bitfun.mobile.app.ui.remote

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.theme.CodeSyntaxColors
import com.bitfun.mobile.app.ui.theme.bitFunColors
import com.bitfun.mobile.core.feature.workspace.CodeSyntaxHighlighter
import com.bitfun.mobile.core.feature.workspace.CodeSyntaxTokenKind
import com.bitfun.mobile.core.feature.workspace.FilePreviewFailureKind
import com.bitfun.mobile.core.feature.workspace.RemoteFilePreviewUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceIntent
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState

internal const val FILE_PREVIEW_TEST_TAG: String = "file-preview"

/**
 * A file the agent referenced, ported from `pages/components/FilePreviewSurface.ets`.
 *
 * The card is a peek, not an editor: text arrives already capped by the shared
 * `FilePreviewPolicy`, so the truncation notice is part of the content rather
 * than an error. Every failure names what went wrong and offers Retry only when
 * retrying could work — a file outside the workspace will not appear on a second
 * try, and a Retry button there is a lie.
 */
@Composable
internal fun FilePreviewSurface(
    preview: RemoteFilePreviewUiState,
    onIntent: (RemoteWorkspaceIntent) -> Unit,
    modifier: Modifier,
) {
    if (preview is RemoteFilePreviewUiState.None) return

    Card(modifier = modifier.fillMaxWidth().testTag(FILE_PREVIEW_TEST_TAG)) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        preview.displayName(),
                        style = MaterialTheme.typography.titleSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    preview.lineRange()?.let { range ->
                        Text(
                            range,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                TextButton(onClick = { onIntent(RemoteWorkspaceIntent.DismissPreview) }) {
                    Text(stringResource(R.string.file_preview_close))
                }
            }

            when (preview) {
                RemoteFilePreviewUiState.None -> Unit
                is RemoteFilePreviewUiState.Loading -> CircularProgressIndicator()

                is RemoteFilePreviewUiState.Text -> {
                    val code = bitFunColors.code
                    // Lexing is linear over the whole file, so it is not
                    // something to redo on every recomposition — scrolling the
                    // card is exactly when it would happen.
                    val source = remember(preview, code) {
                        highlightedSource(
                            content = preview.content,
                            fileName = preview.name.ifEmpty { preview.target.path },
                            lineStart = preview.target.lineStart,
                            lineEnd = preview.target.lineEnd,
                            colors = code,
                        )
                    }
                    // Source lines are not reflowed: a wrapped line number or a
                    // broken indent makes a diff unreadable, so it scrolls.
                    Text(
                        source,
                        style = MaterialTheme.typography.bodySmall,
                        fontFamily = FontFamily.Monospace,
                        softWrap = false,
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(max = 320.dp)
                            .horizontalScroll(rememberScrollState()),
                    )
                    if (preview.truncated) {
                        Text(
                            stringResource(R.string.file_preview_truncated),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                is RemoteFilePreviewUiState.Image -> {
                    val bitmap = remember(preview.bytes) {
                        BitmapFactory.decodeByteArray(preview.bytes, 0, preview.bytes.size)
                    }
                    if (bitmap != null) {
                        Image(
                            bitmap = bitmap.asImageBitmap(),
                            contentDescription = preview.name,
                            contentScale = ContentScale.Fit,
                            modifier = Modifier.fillMaxWidth().heightIn(max = 360.dp),
                        )
                    } else {
                        // The bytes arrived but this device cannot decode them;
                        // that is a decode failure, not a transfer failure.
                        Text(
                            stringResource(R.string.file_preview_unsupported, preview.mimeType),
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                }

                is RemoteFilePreviewUiState.Unsupported -> Text(
                    stringResource(R.string.file_preview_unsupported, preview.mimeType),
                    style = MaterialTheme.typography.bodySmall,
                )

                is RemoteFilePreviewUiState.Failed -> {
                    Text(
                        stringResource(preview.kind.messageRes()),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    if (preview.retryable) {
                        TextButton(
                            onClick = {
                                onIntent(
                                    RemoteWorkspaceIntent.OpenFile(
                                        preview.target.path,
                                        preview.target.displayName,
                                        preview.target.sessionId,
                                    ),
                                )
                            },
                        ) { Text(stringResource(R.string.chat_retry)) }
                    }
                }
            }
        }
    }
}

/**
 * Colours the preview and marks the lines the agent's reference named.
 *
 * The shared [CodeSyntaxHighlighter] decides *what* each run is; this decides
 * what that looks like. Plain runs are left unspecified so they inherit the
 * card's own text colour rather than pinning a second definition of "ink".
 */
private fun highlightedSource(
    content: String,
    fileName: String,
    lineStart: Int,
    lineEnd: Int,
    colors: CodeSyntaxColors,
): AnnotatedString = buildAnnotatedString {
    for (token in CodeSyntaxHighlighter.tokenize(content, fileName)) {
        val style = SpanStyle(
            color = token.kind.color(colors),
            background = if (isTargetLine(token.lineNumber, lineStart, lineEnd)) {
                colors.targetBackground
            } else {
                Color.Unspecified
            },
        )
        withStyle(style) { append(token.text) }
    }
}

private fun CodeSyntaxTokenKind.color(colors: CodeSyntaxColors): Color = when (this) {
    CodeSyntaxTokenKind.PLAIN -> Color.Unspecified
    CodeSyntaxTokenKind.LINE_NUMBER -> colors.lineNumber
    CodeSyntaxTokenKind.KEYWORD -> colors.keyword
    CodeSyntaxTokenKind.STRING -> colors.string
    CodeSyntaxTokenKind.NUMBER -> colors.number
    CodeSyntaxTokenKind.COMMENT -> colors.comment
    CodeSyntaxTokenKind.FUNCTION -> colors.function
    CodeSyntaxTokenKind.TYPE -> colors.type
    CodeSyntaxTokenKind.CONSTANT -> colors.constant
    CodeSyntaxTokenKind.PROPERTY -> colors.property
}

/**
 * Line `0` means the file was too big to lex and came back as one block, so
 * there is nothing to line the highlight up with — better none than the wrong rows.
 */
private fun isTargetLine(lineNumber: Int, lineStart: Int, lineEnd: Int): Boolean {
    if (lineNumber <= 0 || lineStart <= 0) return false
    return lineNumber in lineStart..maxOf(lineEnd, lineStart)
}

/** The desktop's own sentence never reaches here; each reason has our wording. */
private fun FilePreviewFailureKind.messageRes(): Int = when (this) {
    FilePreviewFailureKind.NOT_FOUND -> R.string.file_preview_not_found
    FilePreviewFailureKind.UNAVAILABLE -> R.string.file_preview_unavailable
    FilePreviewFailureKind.ACCESS_DENIED -> R.string.file_preview_access_denied
    FilePreviewFailureKind.TOO_LARGE -> R.string.file_preview_too_large
    FilePreviewFailureKind.CONNECTION -> R.string.file_preview_connection
    FilePreviewFailureKind.LOAD_FAILED -> R.string.file_preview_failed
}

private fun RemoteFilePreviewUiState.displayName(): String = when (this) {
    RemoteFilePreviewUiState.None -> ""
    is RemoteFilePreviewUiState.Loading -> target.displayName
    is RemoteFilePreviewUiState.Text -> name
    is RemoteFilePreviewUiState.Image -> name
    is RemoteFilePreviewUiState.Unsupported -> target.displayName
    is RemoteFilePreviewUiState.Failed -> target.displayName
}

/** `path:12-40`, when the reference the agent wrote named lines. */
private fun RemoteFilePreviewUiState.lineRange(): String? {
    val target = when (this) {
        RemoteFilePreviewUiState.None -> null
        is RemoteFilePreviewUiState.Loading -> target
        is RemoteFilePreviewUiState.Text -> target
        is RemoteFilePreviewUiState.Image -> target
        is RemoteFilePreviewUiState.Unsupported -> target
        is RemoteFilePreviewUiState.Failed -> target
    } ?: return null
    if (target.lineStart <= 0) return null
    return if (target.lineEnd > target.lineStart) {
        "L${target.lineStart}-${target.lineEnd}"
    } else {
        "L${target.lineStart}"
    }
}

/**
 * The file this surface is currently holding, as the remote path the projector
 * also produces, so a file card under a turn can mark itself as the open one.
 *
 * Empty when nothing is open — an empty path matches no card, which is the same
 * answer as "none of them" without a nullable travelling down to every bubble.
 */
internal fun RemoteWorkspaceUiState.previewingRemotePath(): String {
    val preview = (this as? RemoteWorkspaceUiState.Ready)?.preview ?: return ""
    return when (preview) {
        RemoteFilePreviewUiState.None -> ""
        is RemoteFilePreviewUiState.Loading -> preview.target.remotePath
        is RemoteFilePreviewUiState.Text -> preview.target.remotePath
        is RemoteFilePreviewUiState.Image -> preview.target.remotePath
        is RemoteFilePreviewUiState.Unsupported -> preview.target.remotePath
        // A failure still names its file: the card that asked for it should keep
        // showing as the selected one while the reason sits above it.
        is RemoteFilePreviewUiState.Failed -> preview.target.remotePath
    }
}

/** Whether the open file is still on its way, for the card's spinner. */
internal fun RemoteWorkspaceUiState.previewLoading(): Boolean =
    (this as? RemoteWorkspaceUiState.Ready)?.preview is RemoteFilePreviewUiState.Loading
