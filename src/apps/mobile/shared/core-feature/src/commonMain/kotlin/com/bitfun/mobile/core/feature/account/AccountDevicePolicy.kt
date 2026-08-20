package com.bitfun.mobile.core.feature.account

/**
 * Which of an account's registered devices can be driven from this one.
 *
 * Ports the `desktopDevices()` / `canSelectAccountDevice()` pair that
 * `ConnectAccountDevicePage.ets` and `RemoteControlSettingsSheet.ets` both
 * apply — the same rule in both places, which is why it is a rule and not a
 * detail of either screen.
 *
 * The relay's device record carries a name and a presence flag and no kind, so
 * "is this a desktop" has to be answered from the name. That answer is exact for
 * the clients we ship — a HarmonyOS phone registers under a fixed name — and
 * absent for the ones we do not: an Android phone registers under `Build.MODEL`,
 * so a *second* Android install shows up here looking like a desktop. Only the
 * relay can close that gap by reporting a kind; until it does, this is the same
 * list HarmonyOS shows, which is the point.
 */
public object AccountDevicePolicy {
    /**
     * Names our own mobile clients register under.
     *
     * A phone is never a control target — it is the thing doing the
     * controlling — and the current device is excluded separately by id, so this
     * set is about the user's *other* phones.
     */
    private val MOBILE_CLIENT_NAMES: Set<String> = setOf("HarmonyOS Phone")

    /**
     * The rows a device list shows: everything the account has registered except
     * this device and the user's other phones.
     *
     * Offline devices stay in the list. They are what tells a user the desktop
     * they are looking for is known to the account and simply not running, which
     * is a different problem from it never having been paired.
     */
    public fun controlTargets(
        devices: List<AccountDeviceUi>,
        currentDeviceId: String,
    ): List<AccountDeviceUi> = devices.filter { device ->
        device.id != currentDeviceId && device.name !in MOBILE_CLIENT_NAMES
    }

    /** Whether tapping [device] can do anything. */
    public fun canSelect(device: AccountDeviceUi, currentDeviceId: String): Boolean =
        device.online && device.id != currentDeviceId && device.name !in MOBILE_CLIENT_NAMES

    /**
     * The device to drive when the user has not picked one — after a fresh sign
     * in, where a list of one online desktop should not need a second tap.
     *
     * Null when nothing is online, rather than the first offline row: a target
     * that cannot answer is worse than no target, because the session list would
     * then fail against it instead of saying there is nowhere to look.
     */
    public fun preferredTarget(
        devices: List<AccountDeviceUi>,
        currentDeviceId: String,
    ): AccountDeviceUi? = controlTargets(devices, currentDeviceId).firstOrNull { it.online }
}
