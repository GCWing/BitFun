package com.bitfun.mobile.app.platform

import android.content.Context
import android.os.Build
import com.bitfun.mobile.core.feature.pairing.DeviceIdentity
import java.util.UUID

private const val PREFS = "bitfun_install"
private const val KEY_INSTALL_ID = "install_id"

internal val LEGACY_MOBILE_DEVICE_NAMES: Set<String> = setOf(
    "HarmonyOS Phone",
    "HarmonyOS Watch",
)

/**
 * The identity this install presents to the desktop.
 *
 * Deliberately not `ANDROID_ID` or any other hardware-derived value: the desktop
 * only needs something stable enough to recognise the same phone twice, and a
 * random id per install gives it that without carrying a device fingerprint off
 * the phone. Clearing app data intentionally produces a new device.
 */
internal fun Context.deviceIdentity(): DeviceIdentity {
    val prefs = getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    val existing = prefs.getString(KEY_INSTALL_ID, null)
    val installId = existing ?: "android-${UUID.randomUUID()}".also {
        prefs.edit().putString(KEY_INSTALL_ID, it).apply()
    }
    return DeviceIdentity(
        installId = installId,
        displayName = Build.MODEL.ifBlank { "Android" },
    )
}
