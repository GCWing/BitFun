package com.openbitfun.mobile.core.feature.account

import com.openbitfun.mobile.core.transport.accountDeviceLink

public enum class AccountDeviceLinkStatus { INVALID, SIGN_IN_REQUIRED, UNAVAILABLE, READY }

public data class AccountDeviceLinkResult(public val status: AccountDeviceLinkStatus, public val deviceId: String?, public val relayUrl: String? = null)

/** Membership must come from the account signed in at this exact Relay endpoint. */
public fun resolveAccountDeviceLink(url: String, state: AccountUiState): AccountDeviceLinkResult {
    val link = accountDeviceLink(url)
        ?: return AccountDeviceLinkResult(AccountDeviceLinkStatus.INVALID, null)
    val id = link.deviceId
    val ready = (state as? AccountUiState.Ready)?.takeIf { it.relayUrl == link.relayUrl }
        ?: return AccountDeviceLinkResult(AccountDeviceLinkStatus.SIGN_IN_REQUIRED, id, link.relayUrl)
    return if (ready.devices.any { it.id == id && it.online }) {
        AccountDeviceLinkResult(AccountDeviceLinkStatus.READY, id, link.relayUrl)
    } else AccountDeviceLinkResult(AccountDeviceLinkStatus.UNAVAILABLE, null)
}
