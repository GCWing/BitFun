package com.bitfun.mobile.app.ui.chat

import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
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
import com.bitfun.mobile.core.feature.workspace.RemoteFileDownloadUiState

/** Timeline renderer over feature-owned presentation rows; session routing stays above it. */
@Composable
internal fun ConversationTimelineView(
    rows: List<ConversationRow>,
    enabled: Boolean,
    onApproveTool: (String) -> Unit,
    onRejectTool: (String, String) -> Unit,
    onCancelTool: (String, String) -> Unit,
    onAnswerTool: (String, String) -> Unit,
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
                when {
                    !canScrollForward -> stickToBottom = true
                    scrolling -> stickToBottom = false
                }
            }
    }
    LaunchedEffect(rows, stickToBottom) {
        if (stickToBottom && rows.isNotEmpty()) {
            listState.scrollToItem(rows.lastIndex)
            while (listState.canScrollForward) {
                val viewport = listState.layoutInfo.viewportSize.height.coerceAtLeast(1)
                if (listState.scrollBy(viewport * 8f) <= 0f) break
            }
        }
    }

    Box(modifier = modifier) {
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().testTag(CONVERSATION_LIST_TEST_TAG),
            contentPadding = PaddingValues(start = 20.dp, end = 20.dp, bottom = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.Bottom),
        ) {
            items(rows, key = { it.id }) { row ->
                ChatMessageBubble(
                    row = row,
                    enabled = enabled,
                    onApproveTool = onApproveTool,
                    onRejectTool = onRejectTool,
                    onCancelTool = onCancelTool,
                    onAnswerTool = onAnswerTool,
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
