package com.bitfun.mobile.core.feature.pairing

import com.bitfun.mobile.core.feature.CoreLog
import com.bitfun.mobile.core.persistence.iosSecureStore
import kotlinx.coroutines.CoroutineScope

/** iOS pairing wiring; the credential cooldown lives in the Keychain. */
public fun PairingStore.Companion.create(
    scope: CoroutineScope,
    device: DeviceIdentity,
    log: CoreLog,
): PairingStore = PairingStore.create(
    scope = scope,
    device = device,
    protection = iosSecureStore("com.bitfun.mobile.pairing"),
    log = log,
)
