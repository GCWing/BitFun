package com.openbitfun.mobile.core.feature.account

import com.openbitfun.mobile.core.feature.CoreLog
import com.openbitfun.mobile.core.persistence.iosPersistenceStores
import com.openbitfun.mobile.core.persistence.iosSecureStore
import kotlinx.coroutines.CoroutineScope

/** iOS account wiring; credentials and master keys stay in the Keychain. */
public fun AccountStore.Companion.create(
    scope: CoroutineScope,
    service: String,
    deviceId: String,
    deviceName: String,
    log: CoreLog,
): AccountStore {
    val persistence = iosPersistenceStores("openbitfun-mobile.db")
    return AccountStore.create(
        scope = scope,
        backend = AccountStore.backend(log, emptySet()),
        secureStore = iosSecureStore(service),
        deviceId = deviceId,
        deviceName = deviceName,
        persistence = persistence,
    )
}
