package com.bitfun.mobile.app.ui.chat

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.chat.message.ChatCaption
import com.bitfun.mobile.app.ui.chat.message.ChatMessageRetryAction
import com.bitfun.mobile.app.ui.chat.message.ChatTypingDots
import com.bitfun.mobile.app.ui.chat.message.ChatUserMessageBubble
import com.bitfun.mobile.app.ui.chat.message.MessageBlockCallbacks
import com.bitfun.mobile.app.ui.chat.message.MessageBlockList
import com.bitfun.mobile.app.ui.chat.message.MessageImageGallery
import com.bitfun.mobile.app.ui.chat.message.ThinkingBlock
import com.bitfun.mobile.app.ui.chat.tool.ToolStatusList
import com.bitfun.mobile.core.feature.session.ConversationRow
import com.bitfun.mobile.core.feature.session.QuestionAnswer
import com.bitfun.mobile.core.feature.session.ConversationRowKind
import com.bitfun.mobile.core.feature.workspace.RemoteFileDownloadUiState

internal const val BUBBLE_TEST_TAG: String = "chat-bubble"

/**
 * One turn, ported from `pages/components/ChatMessageBubble.ets`.
 *
 * The user's own words sit right and tinted; the agent's sit left on the plain
 * surface, because an agent turn can be long and reversed contrast is tiring to
 * read. Reasoning and tools hang off the agent side only.
 */
@Composable
internal fun ChatMessageBubble(
    row: ConversationRow,
    enabled: Boolean,
    onApproveTool: (String) -> Unit,
    onRejectTool: (String, String) -> Unit,
    onCancelTool: (String, String) -> Unit,
    onAnswerTool: (String, String) -> Unit,
    onAnswerToolStructured: (String, List<QuestionAnswer>) -> Unit,
    onRetry: (String) -> Unit,
    onOpenLink: (String, String) -> Unit,
    /** The file the preview surface is showing, so its card can say so. */
    previewingRemotePath: String,
    previewLoading: Boolean,
    download: RemoteFileDownloadUiState = RemoteFileDownloadUiState.None,
    onDownloadFile: (String, String) -> Unit = { _, _ -> },
    downloadEnabled: Boolean = true,
    modifier: Modifier,
) {
    val fromUser = row.kind == ConversationRowKind.USER
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(
                top = if (fromUser) 8.dp else 2.dp,
                bottom = if (fromUser) 12.dp else 10.dp,
            )
            .testTag(BUBBLE_TEST_TAG),
        horizontalAlignment = if (fromUser) Alignment.End else Alignment.Start,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (fromUser) {
            ChatUserMessageBubble(row.text, row.images, Modifier)
        } else {
            AssistantContent(
                row = row,
                callbacks = MessageBlockCallbacks(
                    enabled = enabled,
                    onApproveTool = onApproveTool,
                    onRejectTool = onRejectTool,
                    onCancelTool = onCancelTool,
                    onAnswerTool = onAnswerTool,
                    onAnswerToolStructured = onAnswerToolStructured,
                    onOpenLink = onOpenLink,
                    previewingRemotePath = previewingRemotePath,
                    previewLoading = previewLoading,
                    download = download,
                    onDownloadFile = onDownloadFile,
                    downloadEnabled = downloadEnabled,
                ),
            )
            MessageImageGallery(images = row.images, userStyle = false)
        }

        // Three mutually exclusive footnotes about delivery, in the order they
        // can happen: nothing has arrived yet, sent but unacknowledged, refused.
        when {
            row.showRetry -> ChatMessageRetryAction(
                fromUser = fromUser,
                enabled = enabled,
                onRetry = { onRetry(row.text) },
                modifier = Modifier,
            )

            row.typing -> ChatTypingDots(Modifier)
            row.pending -> ChatCaption(stringResource(R.string.chat_pending), error = false)
        }
    }
}

/**
 * The agent's side, drawn either as the ordered turn the agent produced or, when
 * it produced no such structure, as the plain answer it is.
 *
 * The fork is the shared layer's: [ConversationRow.blocks] is populated only for
 * turns whose ordering carries meaning, and it already contains everything the
 * flat fields would have shown.
 */
@Composable
private fun AssistantContent(row: ConversationRow, callbacks: MessageBlockCallbacks) {
    if (row.blocks.isNotEmpty()) {
        MessageBlockList(row.blocks, callbacks, Modifier)
        return
    }

    row.thinking?.let { ThinkingBlock(it, row.streaming, row.id) }
    if (row.text.isNotEmpty()) {
        // No bubble on this side, matching `ChatMessageChrome.ets`: an agent
        // turn carries headings, lists and code cards, and a rounded tint
        // around all of that reads as one quoted lump.
        MarkdownContent(
            text = row.text,
            onOpenLink = callbacks.onOpenLink,
            modifier = Modifier,
            streaming = row.streaming,
        )
        FileReferenceCards(
            text = row.text,
            previewingRemotePath = callbacks.previewingRemotePath,
            previewLoading = callbacks.previewLoading,
            download = callbacks.download,
            onOpen = callbacks.onOpenLink,
            onDownload = callbacks.onDownloadFile,
            downloadEnabled = callbacks.downloadEnabled,
            modifier = Modifier,
        )
    }
    if (row.tools.isNotEmpty()) {
        ToolStatusList(
            tools = row.tools,
            enabled = callbacks.enabled,
            onApprove = callbacks.onApproveTool,
            onReject = callbacks.onRejectTool,
            onCancel = callbacks.onCancelTool,
            onAnswer = callbacks.onAnswerTool,
            onAnswerStructured = callbacks.onAnswerToolStructured,
            onOpenFile = callbacks.onOpenLink,
            modifier = Modifier,
        )
    }
}

/** The invitation shown when a session has no messages at all. */
@Composable
internal fun ConversationEmptyState(modifier: Modifier) {
    Column(
        modifier = modifier.fillMaxWidth().padding(vertical = 32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(stringResource(R.string.chat_empty_title), style = MaterialTheme.typography.titleMedium)
        Text(
            stringResource(R.string.chat_empty_hint),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
