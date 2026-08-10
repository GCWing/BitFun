package com.bitfun.mobile.app.ui.settings

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import com.bitfun.mobile.app.R

internal const val QR_CONNECT_TEST_TAG: String = "qr-connect-entry"

/**
 * The way to a desktop that the account cannot reach, ported from
 * `OtherConnectionSection()`.
 *
 * One entry rather than a list: an account device is the ordinary path, and this
 * is the exception the source names in its own body text — a temporary pairing,
 * or a desktop nobody has signed in on. It leads to the pairing surface, where
 * the camera and the link field already live.
 */
@Composable
internal fun QrConnectEntry(onConnect: () -> Unit, modifier: Modifier) {
    SettingsCard(modifier = modifier.testTag(QR_CONNECT_TEST_TAG)) {
        SettingsEntryRow(
            title = stringResource(R.string.remote_settings_qr_connect),
            subtitle = stringResource(R.string.remote_settings_qr_connect_body),
            icon = R.drawable.ic_symbol_link,
            minHeight = 78,
            onClick = onConnect,
            modifier = Modifier,
        )
    }
}
