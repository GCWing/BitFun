package com.bitfun.mobile.app.ui.chat

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.common.chatStatusBarCopy
import com.bitfun.mobile.app.ui.theme.bitFunColors
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.connection.ConnectionStatusPresenter
import com.bitfun.mobile.core.feature.connection.ConnectionTone

internal const val CHAT_STATUS_BAR_TEST_TAG: String = "chat-status-bar"
internal const val CHAT_STATUS_DOT_TEST_TAG: String = "chat-status-dot"
internal const val CHAT_STATUS_STOP_TEST_TAG: String = "chat-status-stop"

/** The relay state strip between the conversation header and transcript. */
@Composable
internal fun ChatStatusBar(
    phase: ConnectionPhase,
    canStop: Boolean,
    onStop: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val line = MaterialTheme.colorScheme.outlineVariant
    val copy = phase.chatStatusBarCopy(canStop)
    val title = stringResource(copy.title)
    val detail = stringResource(copy.detail)
    val stopLabel = stringResource(R.string.message_stop)
    val statusLabel = if (detail != title) "$title · $detail" else title
    val statusColor = when (ConnectionStatusPresenter.tone(phase)) {
        ConnectionTone.OK -> bitFunColors.statusSuccess
        ConnectionTone.BUSY -> MaterialTheme.colorScheme.onSurfaceVariant
        ConnectionTone.ERROR -> MaterialTheme.colorScheme.error
        ConnectionTone.MUTED -> MaterialTheme.colorScheme.onSurfaceVariant
    }
    Row(
        modifier = modifier
            .testTag(CHAT_STATUS_BAR_TEST_TAG)
            .fillMaxWidth()
            .height(48.dp)
            .drawBehind {
                val stroke = 1.dp.toPx()
                drawLine(line, start = androidx.compose.ui.geometry.Offset.Zero, end = androidx.compose.ui.geometry.Offset(size.width, 0f), strokeWidth = stroke)
                drawLine(line, start = androidx.compose.ui.geometry.Offset(0f, size.height), end = androidx.compose.ui.geometry.Offset(size.width, size.height), strokeWidth = stroke)
            }
            .padding(horizontal = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (canStop) Spacer(Modifier.width(68.dp))
        Row(
            modifier = Modifier.weight(1f),
            horizontalArrangement = if (canStop) Arrangement.Center else Arrangement.Start,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                Modifier
                    .testTag(CHAT_STATUS_DOT_TEST_TAG)
                    .size(8.dp)
                    .background(statusColor, CircleShape),
            )
            Spacer(Modifier.width(7.dp))
            Text(
                statusLabel,
                fontSize = 13.sp,
                color = if (canStop) MaterialTheme.colorScheme.onSurfaceVariant else statusColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        if (canStop) {
            Box(
                modifier = Modifier
                    .width(68.dp)
                    .height(34.dp)
                    .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(17.dp))
                    .clickable(role = Role.Button, onClick = onStop)
                    .semantics {
                        contentDescription = stopLabel
                        role = Role.Button
                    }
                    .testTag(CHAT_STATUS_STOP_TEST_TAG),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    stopLabel,
                    fontSize = 13.sp,
                    fontWeight = FontWeight.Medium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
    }
}
