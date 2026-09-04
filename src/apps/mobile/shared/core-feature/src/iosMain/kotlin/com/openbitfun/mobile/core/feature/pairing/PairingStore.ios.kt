package com.openbitfun.mobile.core.feature.pairing

import com.openbitfun.mobile.core.feature.CoreLog
import com.openbitfun.mobile.core.persistence.iosPersistenceStores
import com.openbitfun.mobile.core.persistence.iosSecureStore
import kotlinx.coroutines.CoroutineScope

/** iOS pairing wiring; the credential cooldown lives in the Keychain. */
public fun PairingStore.Companion.create(
    scope: CoroutineScope,
    device: DeviceIdentity,
    log: CoreLog,
): PairingStore {
    val persistence = iosPersistenceStores("openbitfun-mobile.db")
    return PairingStore.create(
        scope = scope,
        device = device,
        protection = iosSecureStore("com.openbitfun.mobile.pairing"),
        log = log,
        persistence = persistence,
    )
}
