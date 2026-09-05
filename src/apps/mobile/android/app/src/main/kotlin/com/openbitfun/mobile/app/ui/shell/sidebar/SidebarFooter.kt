package com.openbitfun.mobile.app.ui.shell.sidebar

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.app.ui.common.SignedOutConnectionActions

internal const val SIDEBAR_NEW_CHAT_TEST_TAG: String = "app-sidebar-new-chat"
internal const val SIDEBAR_SETTINGS_TEST_TAG: String = "app-sidebar-settings"

/**
 * The signed-in footer, ported from `AuthenticatedFooter` in `AppSidebar.ets`.
 *
 * Carded and floating over the list rather than docked below it: the list scrolls
 * behind it, so starting a new conversation stays reachable no matter how far
 * down the history a user has gone.
 */
@Composable
internal fun SidebarAuthenticatedFooter(onNewChat: () -> Unit, onOpenSettings: () -> Unit) {
    val newChatLabel = stringResource(R.string.sidebar_new_chat)
    Row(
        modifier = Modifier.fillMaxWidth().height(56.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            modifier = Modifier
                .width(116.dp)
                .height(46.dp)
                .shadow(2.dp, RoundedCornerShape(23.dp))
                .clip(RoundedCornerShape(23.dp))
                .background(MaterialTheme.colorScheme.surface)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(23.dp))
                .clickable(role = Role.Button, onClick = onNewChat)
                .semantics(mergeDescendants = true) {
                    contentDescription = newChatLabel
                }
                .testTag(SIDEBAR_NEW_CHAT_TEST_TAG),
            horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                painterResource(R.drawable.ic_symbol_square_and_pencil),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.size(18.dp),
            )
            Text(
                newChatLabel,
                fontSize = 15.sp,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
        Box(Modifier.weight(1f))
        SidebarCircleButton(
            icon = R.drawable.ic_symbol_gearshape,
            contentDescription = stringResource(R.string.navigation_settings),
            diameter = 46,
            onClick = onOpenSettings,
            modifier = Modifier.testTag(SIDEBAR_SETTINGS_TEST_TAG),
        )
    }
}

/**
 * The signed-out footer: one full-width call to sign in, the way `SignedOutFooter`
 * draws it. Filled rather than carded because it is the only thing to press here.
 */
@Composable
internal fun SidebarSignedOutFooter(
    showScan: Boolean,
    onScanDesktop: () -> Unit,
    onOpenAccount: () -> Unit,
) {
    SignedOutConnectionActions(
        scanLabel = stringResource(R.string.sidebar_scan_to_connect),
        accountLabel = stringResource(R.string.sidebar_sign_in),
        onScan = onScanDesktop,
        onOpenAccount = onOpenAccount,
        showScan = showScan,
    )
}
