package com.bitfun.mobile.core.feature.account

import com.bitfun.mobile.core.feature.CoreLog
import com.bitfun.mobile.core.persistence.iosSecureStore
import kotlinx.coroutines.CoroutineScope

/** iOS account wiring; credentials and master keys stay in the Keychain. */
public fun AccountStore.Companion.create(
    scope: CoroutineScope,
    service: String,
    deviceId: String,
    deviceName: String,
    log: CoreLog,
): AccountStore = AccountStore.create(
    scope = scope,
    backend = AccountStore.backend(log),
    secureStore = iosSecureStore(service),
    deviceId = deviceId,
    deviceName = deviceName,
)
