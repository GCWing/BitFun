package com.openbitfun.mobile.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.core.feature.connection.ConnectionPhase
import com.openbitfun.mobile.core.feature.connection.RemoteControlSummary
import com.openbitfun.mobile.core.feature.session.RemoteSessionIntent
import com.openbitfun.mobile.core.feature.session.RemoteSessionUiState

internal const val SETTINGS_PROFILE_TEST_TAG: String = "settings-profile"
internal const val SETTINGS_CLOSE_TEST_TAG: String = "settings-close"

/**
 * The remote-control page, ported from `pages/components/RemoteControlSettingsSheet.ets`.
 *
 * The source reads top to bottom as one answer to "what is this phone driving":
 * who I am, what I am controlling and how that link was made, what other links I
 * could make instead, and what it may run without asking. Nothing else: choosing
 * a theme is a fact about the phone, so it lives on [GeneralSettingsScreen] and
 * every section here names a desktop.
 *
 * @param accountUserId who is signed in, or null. Only to decide whether the row
 * leads to the account or to signing in — the account surface reads its own store.
 * @param summary which desktop this phone is driving, decided by
 * `RemoteControlPresenter` rather than here.
 * @param remoteState the session store of whichever connection [summary] named,
 * read here only for the desktop-wide permission mode.
 * @param onConnectByLink opens the pairing surface, where the camera and the
 * link field already live.
 */
@Composable
internal fun SettingsScreen(
    modifier: Modifier,
    accountUserId: String?,
    summary: RemoteControlSummary,
    remoteState: RemoteSessionUiState,
    onOpenAccount: () -> Unit,
    onDisconnect: () -> Unit,
    onReconnect: () -> Unit,
    onConnectByLink: () -> Unit,
    onClose: () -> Unit,
    onSessionIntent: (RemoteSessionIntent) -> Unit,
) {
    val connected = summary.phase == ConnectionPhase.CONNECTED

    // What `aboutToAppear` does in the source: the mode is the desktop's, so it
    // may have been changed from the desktop or from another phone since the
    // last time this page was opened, and only asking again can say so.
    LaunchedEffect(connected) {
        if (connected) onSessionIntent(RemoteSessionIntent.RefreshPermissionMode)
    }

    Box(modifier = modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(start = 18.dp, end = 18.dp, top = 20.dp, bottom = 42.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text(
                stringResource(R.string.remote_settings_title),
                style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                textAlign = TextAlign.Center,
                // Centred with room kept on both sides for the close button, so
                // the title stays on the page's axis rather than on the axis of
                // what is left over beside the button.
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 52.dp)
                    .padding(top = 8.dp, bottom = 16.dp),
            )

            // The only way back to the account once signed in: the drawer's footer
            // swaps its sign-in button for New chat the moment there is a session,
            // exactly as `AppSidebar.ets` does, and `RemoteControlSettingsSheet.ets`
            // is where that platform puts the account and its devices in exchange.
            // Without this row, signing in is a one-way door — no device switch, no
            // sign out.
            SettingsCard(modifier = Modifier.testTag(SETTINGS_PROFILE_TEST_TAG)) {
                SettingsEntryRow(
                    // No subtitle, as `ProfileEntry()` has none: the row is a
                    // door, and which account is behind it is the first thing
                    // the page behind it says.
                    title = if (accountUserId.isNullOrBlank()) {
                        stringResource(R.string.sidebar_sign_in)
                    } else {
                        stringResource(R.string.remote_settings_profile)
                    },
                    subtitle = "",
                    icon = R.drawable.ic_symbol_person,
                    minHeight = 64,
                    onClick = onOpenAccount,
                    modifier = Modifier,
                )
            }

            SettingsSectionHeader(
                stringResource(R.string.remote_settings_current_control),
                modifier = Modifier.padding(top = 4.dp),
            )
            CurrentControlCard(
                summary = summary,
                onDisconnect = onDisconnect,
                onReconnect = onReconnect,
                modifier = Modifier,
            )

            SettingsSectionHeader(
                stringResource(R.string.remote_settings_other_methods),
                modifier = Modifier.padding(top = 4.dp),
            )
            QrConnectEntry(onConnect = onConnectByLink, modifier = Modifier)

            // Only once the store is Ready is there a mode to show or a command
            // to send: an unpaired phone has no desktop to be permitted by, and
            // the source hides the whole section for the same reason.
            (remoteState as? RemoteSessionUiState.Ready)?.let { ready ->
                PermissionSection(
                    state = ready,
                    connected = connected,
                    onIntent = onSessionIntent,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
        }

        // The source's `CloseButton()`: a floating circle over the top-right of
        // the page. A sheet can already be swiped away, but the swipe starts on
        // content that scrolls, so the gesture and the page disagree about what
        // a downward drag means — the button is the reading that never does.
        FilledTonalIconButton(
            onClick = onClose,
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(top = 16.dp, end = 16.dp)
                .size(44.dp)
                .testTag(SETTINGS_CLOSE_TEST_TAG),
        ) {
            Icon(
                painterResource(R.drawable.ic_symbol_xmark),
                contentDescription = stringResource(R.string.common_close),
                modifier = Modifier.size(20.dp),
            )
        }
    }
}
