package com.bitfun.mobile.core.protocol

/**
 * Contract fact for the provision_peer_device command.
 *
 * provision_peer_device mints a full credential for a keyboard-less companion
 * device (a watch) relayed through a phone. It is not an Android/mobile-phone
 * product capability, so this client exposes no outbound command, response, or
 * intent for it, only this exclusion fact.
 */
public enum class ProvisionPeerDeviceSupport {
    UNSUPPORTED,
}

public object ProvisionPeerDeviceContract {
    public val support: ProvisionPeerDeviceSupport = ProvisionPeerDeviceSupport.UNSUPPORTED
    public val commandName: String = "provision_peer_device"
}
