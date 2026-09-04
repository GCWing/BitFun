package com.openbitfun.mobile.core.crypto

import dev.whyoleg.cryptography.CryptographyProvider
import dev.whyoleg.cryptography.providers.jdk.JDK

/**
 * The JVM target exists for fast host tests, so it uses the platform JDK
 * providers as-is: JDK 17 has had X25519 since 11 and needs no extra provider.
 *
 * Note this is deliberately *not* the same provider as Android's. The Android
 * host-test source set compiles `androidMain`, so BouncyCastle is exercised
 * there, and running the same suite on both means a provider-specific
 * difference shows up as a test failure rather than as a device-only bug.
 */
internal actual val relayCryptographyProvider: CryptographyProvider by lazy {
    CryptographyProvider.JDK
}
