package com.bitfun.mobile.app.ui.common

import androidx.annotation.DrawableRes
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp

/**
 * The floating round control the source draws on a page rather than on a bar.
 *
 * `RemoteChatHeader.ets` and `RemoteCreateSessionView.ets` both open with one of
 * these in the top-left, which is what keeps whatever is centred — a title, or
 * nothing at all — the subject of the screen instead of one of three equal
 * things in a row. Shared rather than copied so the two screens cannot drift
 * apart by a dp.
 */
@Composable
internal fun CircleControl(
    @DrawableRes icon: Int,
    glyphSize: Int,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier,
) {
    Surface(
        onClick = onClick,
        shape = CircleShape,
        color = MaterialTheme.colorScheme.surface,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        shadowElevation = 2.dp,
        modifier = modifier.size(44.dp),
    ) {
        Box(contentAlignment = Alignment.Center) {
            Icon(
                painterResource(icon),
                contentDescription = contentDescription,
                modifier = Modifier.size(glyphSize.dp),
            )
        }
    }
}
