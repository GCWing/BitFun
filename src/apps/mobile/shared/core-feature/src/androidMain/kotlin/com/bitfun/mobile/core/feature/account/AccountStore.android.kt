package com.bitfun.mobile.core.feature.account

import android.content.Context
import com.bitfun.mobile.core.feature.CoreLog
import com.bitfun.mobile.core.persistence.androidPersistenceStores
import com.bitfun.mobile.core.persistence.androidSecureStore
import kotlinx.coroutines.CoroutineScope

/**
 * [log] reaches the account's own transport, which is the only place a device
 * RPC can say why it failed — the screen above it only ever sees a reason code.
 */
public fun AccountStore.Companion.create(
    scope: CoroutineScope,
    context: Context,
    deviceId: String,
    deviceName: String,
    log: CoreLog,
    legacyMobileDeviceNames: Set<String>,
): AccountStore {
    val persistence = androidPersistenceStores(context.applicationContext, "bitfun-mobile.db")
    return AccountStore.create(
        scope = scope,
        backend = AccountStore.backend(log, legacyMobileDeviceNames),
        secureStore = androidSecureStore(context.applicationContext, "cloud_account"),
        deviceId = deviceId,
        deviceName = deviceName,
        persistence = persistence,
    )
}
