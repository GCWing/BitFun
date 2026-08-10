package com.bitfun.mobile.core.feature.pairing

import com.bitfun.mobile.core.transport.RelayDescriptorParser

/**
 * What a pairing link says about the form around it.
 *
 * Ports `RemotePairingPolicy.projection`. A half-typed link is an ordinary
 * state, not an error, so this never throws — the screen calls it on every
 * keystroke to decide whether to show the password field.
 */
public data class PairingLinkHints(
    val requiresAccount: Boolean,
    val suggestedUserId: String,
) {
    public companion object {
        public val None: PairingLinkHints = PairingLinkHints(
            requiresAccount = false,
            suggestedUserId = "",
        )
    }
}

/** Never throws; an unparseable link yields [PairingLinkHints.None]. */
public fun inspectPairingLink(url: String): PairingLinkHints {
    if (url.isBlank()) return PairingLinkHints.None
    return PairingLinkHints(
        requiresAccount = RelayDescriptorParser.accountAuthRequired(url),
        suggestedUserId = RelayDescriptorParser.accountUsername(url),
    )
}
