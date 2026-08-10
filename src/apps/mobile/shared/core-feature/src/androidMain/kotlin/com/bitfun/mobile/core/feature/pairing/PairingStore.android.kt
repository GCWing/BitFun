package com.bitfun.mobile.core.feature.pairing

import android.content.Context
import com.bitfun.mobile.core.feature.CoreLog
import com.bitfun.mobile.core.persistence.androidSecureStore
import kotlinx.coroutines.CoroutineScope

/**
 * The store with its cooldown kept in the platform's keystore-backed store.
 *
 * The app hands over a [Context] rather than a `SecureStore` because everything
 * below `:core-feature` is off-limits to it — see the architecture guardrail in
 * `scripts/check-mobile-architecture.mjs`.
 */
public fun PairingStore.Companion.create(
    scope: CoroutineScope,
    context: Context,
    device: DeviceIdentity,
    log: CoreLog,
): PairingStore = PairingStore.create(
    scope = scope,
    device = device,
    protection = androidSecureStore(context.applicationContext, "pairing_protection"),
    log = log,
)
