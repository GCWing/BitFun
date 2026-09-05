package com.openbitfun.mobile.app.ui.remote

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.core.feature.pairing.PairingFailure
import com.openbitfun.mobile.core.feature.pairing.PairingFailureReason

/**
 * The whole localization seam, in one `when`.
 *
 * The core reports a cause and the app decides the wording, which is why this
 * mapping is exhaustive without an `else`: a reason added upstream breaks the
 * build here rather than silently rendering as blank.
 *
 * It resolves the string rather than returning an id because one of the reasons
 * needs a quantity — [PairingFailureReason.TooManyAttempts] carries how long the
 * cooldown still has to run, and "try again in 1 seconds" is not a sentence.
 */
@Composable
internal fun PairingFailure.message(): String = when (reason) {
    PairingFailureReason.PairingLinkEmpty -> stringResource(R.string.failure_link_empty)
    PairingFailureReason.PairingLinkIncomplete -> stringResource(R.string.failure_link_incomplete)
    PairingFailureReason.PairingLinkUndecodable -> stringResource(R.string.failure_link_undecodable)
    PairingFailureReason.PairingLinkKeyUnusable -> stringResource(R.string.failure_link_key_unusable)
    PairingFailureReason.AccountUsernameRequired -> stringResource(R.string.failure_account_username)
    PairingFailureReason.AccountPasswordRequired -> stringResource(R.string.failure_account_password)
    PairingFailureReason.Rejected -> stringResource(R.string.failure_rejected)
    PairingFailureReason.RoomNotFound -> stringResource(R.string.failure_room_not_found)
    PairingFailureReason.RateLimited -> stringResource(R.string.failure_rate_limited)
    PairingFailureReason.RelayUnavailable -> stringResource(R.string.failure_relay_unavailable)
    PairingFailureReason.NetworkUnreachable -> stringResource(R.string.failure_network)
    PairingFailureReason.Timeout -> stringResource(R.string.failure_timeout)
    PairingFailureReason.ProtocolMismatch -> stringResource(R.string.failure_protocol)
    PairingFailureReason.DesktopRejected -> stringResource(R.string.failure_desktop_rejected)
    PairingFailureReason.TooManyAttempts -> pluralStringResource(
        R.plurals.failure_too_many_attempts,
        retryAfterSeconds,
        retryAfterSeconds,
    )
}
