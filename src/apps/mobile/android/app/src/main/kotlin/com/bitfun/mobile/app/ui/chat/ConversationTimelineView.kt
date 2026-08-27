package com.bitfun.mobile.app.ui.chat

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.core.feature.session.ConversationRow
import com.bitfun.mobile.core.feature.session.QuestionAnswer
import com.bitfun.mobile.core.feature.workspace.RemoteFileDownloadUiState

/** Pure decisions for keeping a forward timeline at its visual tail. */
internal object ConversationScrollPolicy {
    fun shouldStickToBottom(
        currentlySticking: Boolean,
        isAtBottom: Boolean,
        isScrollInProgress: Boolean,
    ): Boolean = when {
        isAtBottom -> true
        isScrollInProgress -> false
        else -> currentlySticking
    }

    fun shouldScrollToBottom(stickToBottom: Boolean, hasRows: Boolean): Boolean =
        stickToBottom && hasRows

    /**
     * The LazyColumn puts the "load older messages" header at index zero when
     * [hasMoreMessages] is true, so the real tail is one past [rowCount] instead
     * of `rowCount - 1`.
     */
    fun lastItemIndex(rowCount: Int, hasMoreMessages: Boolean): Int =
        if (hasMoreMessages) rowCount else (rowCount - 1).coerceAtLeast(0)
}

/** Timeline renderer over feature-owned presentation rows; session routing stays above it. */
@Composable
internal fun ConversationTimelineView(
    rows: List<ConversationRow>,
    hasMoreMessages: Boolean,
    onLoadOlder: () -> Unit,
    enabled: Boolean,
    onApproveTool: (String) -> Unit,
    onRejectTool: (String, String) -> Unit,
    onCancelTool: (String, String) -> Unit,
    onAnswerTool: (String, String) -> Unit,
    onAnswerToolStructured: (String, List<QuestionAnswer>) -> Unit,
    onRetry: (String) -> Unit,
    onOpenFile: (String, String) -> Unit,
    previewingRemotePath: String,
    previewLoading: Boolean,
    download: RemoteFileDownloadUiState,
    onDownloadFile: (String, String) -> Unit,
    downloadEnabled: Boolean,
    modifier: Modifier,
) {
    val listState = rememberLazyListState()
    var stickToBottom by rememberSaveable { mutableStateOf(true) }
    val atBottom by remember(listState) { derivedStateOf { !listState.canScrollForward } }

    LaunchedEffect(listState) {
        snapshotFlow { listState.isScrollInProgress to listState.canScrollForward }
            .collect { (scrolling, canScrollForward) ->
                stickToBottom = ConversationScrollPolicy.shouldStickToBottom(
                    currentlySticking = stickToBottom,
                    isAtBottom = !canScrollForward,
                    isScrollInProgress = scrolling,
                )
            }
    }
    LaunchedEffect(rows, stickToBottom, hasMoreMessages) {
        if (ConversationScrollPolicy.shouldScrollToBottom(stickToBottom, rows.isNotEmpty())) {
            // A large offset positions the item's bottom at the viewport tail directly;
            // unlike scrollToItem(index), it does not briefly expose the item's top.
            listState.scrollToItem(
                ConversationScrollPolicy.lastItemIndex(rows.size, hasMoreMessages),
                scrollOffset = Int.MAX_VALUE,
            )
        }
    }

    Box(modifier = modifier) {
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().testTag(CONVERSATION_LIST_TEST_TAG),
            contentPadding = PaddingValues(start = 20.dp, end = 20.dp, bottom = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.Bottom),
        ) {
            if (hasMoreMessages) {
                item(key = "load-older-messages") {
                    Box(modifier = Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                        TextButton(onClick = onLoadOlder, enabled = enabled) {
                            Text(stringResource(R.string.chat_load_older_messages))
                        }
                    }
                }
            }
            items(rows, key = { it.id }) { row ->
                ChatMessageBubble(
                    row = row,
                    enabled = enabled,
                    onApproveTool = onApproveTool,
                    onRejectTool = onRejectTool,
                    onCancelTool = onCancelTool,
                    onAnswerTool = onAnswerTool,
                    onAnswerToolStructured = onAnswerToolStructured,
                    onRetry = onRetry,
                    onOpenLink = onOpenFile,
                    previewingRemotePath = previewingRemotePath,
                    previewLoading = previewLoading,
                    download = download,
                    onDownloadFile = onDownloadFile,
                    downloadEnabled = downloadEnabled,
                    modifier = Modifier,
                )
            }
        }
        if (!atBottom) {
            Surface(
                onClick = { stickToBottom = true },
                shape = CircleShape,
                color = MaterialTheme.colorScheme.surface,
                shadowElevation = 5.dp,
                tonalElevation = 1.dp,
                modifier = Modifier.align(Alignment.BottomCenter).offset(y = (-4).dp).size(42.dp),
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Icon(
                        painterResource(R.drawable.ic_symbol_chevron_down),
                        contentDescription = stringResource(R.string.chat_scroll_to_bottom),
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        }
    }
}
