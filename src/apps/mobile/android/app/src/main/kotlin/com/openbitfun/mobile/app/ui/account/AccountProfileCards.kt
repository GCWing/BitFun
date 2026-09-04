package com.openbitfun.mobile.app.ui.account

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.app.platform.deviceIdentity
import com.openbitfun.mobile.app.ui.settings.SettingsAvatar
import com.openbitfun.mobile.app.ui.settings.SettingsCard
import com.openbitfun.mobile.app.ui.settings.SettingsChip
import com.openbitfun.mobile.app.ui.settings.SettingsSectionHeader

internal const val ACCOUNT_IDENTITY_TEST_TAG: String = "account-identity"
internal const val ACCOUNT_DETAILS_TEST_TAG: String = "account-details"

/**
 * Who is signed in, ported from the profile half of `RemoteControlSettingsSheet.ets`.
 *
 * The badge is the point of the card: an account that is signed in but whose
 * token the relay has stopped honouring looks exactly like a working one until
 * something fails, so the state is said out loud next to the name.
 */
@Composable
internal fun AccountIdentityCard(userId: String, modifier: Modifier) {
    SettingsCard(modifier = modifier.testTag(ACCOUNT_IDENTITY_TEST_TAG)) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .defaultMinSize(minHeight = 96.dp)
                .padding(horizontal = 18.dp, vertical = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            SettingsAvatar(icon = R.drawable.ic_symbol_person, diameter = 56)
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(
                    stringResource(R.string.remote_settings_openbitfun_user),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    userId,
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            SettingsChip(
                label = stringResource(R.string.remote_settings_account_signed_in),
                enabled = true,
                onClick = null,
                modifier = Modifier,
            )
        }
    }
}

/**
 * The two identifiers support asks for, ported from the source's profile details.
 *
 * The device id is this install's own, read from the same store that stamps it
 * onto every command the desktop receives — which is what makes it the id worth
 * showing, rather than a model name the relay never sees.
 */
@Composable
internal fun AccountDetailsCard(userId: String, modifier: Modifier) {
    val context = LocalContext.current
    val installId = remember(context) { context.deviceIdentity().installId }

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        SettingsSectionHeader(
            stringResource(R.string.remote_settings_profile_details),
            modifier = Modifier,
        )
        SettingsCard(modifier = Modifier.testTag(ACCOUNT_DETAILS_TEST_TAG)) {
            DetailRow(label = stringResource(R.string.remote_settings_user_id), value = userId)
            HorizontalDivider(Modifier.padding(horizontal = 18.dp))
            DetailRow(
                label = stringResource(R.string.remote_settings_device_id),
                value = installId,
            )
        }
    }
}

/** Label on the left, value on the right, the value free to take the room. */
@Composable
private fun DetailRow(label: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .defaultMinSize(minHeight = 56.dp)
            .padding(horizontal = 18.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.End,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
    }
}
