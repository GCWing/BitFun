package com.bitfun.mobile.core.crypto

import dev.whyoleg.cryptography.CryptographyProvider
import dev.whyoleg.cryptography.providers.jdk.JDK
import org.bouncycastle.jce.provider.BouncyCastleProvider

/**
 * Android routes through a BouncyCastle instance we construct ourselves.
 *
 * Two reasons, both about the API 26 floor. Android's platform providers gained
 * X25519 only in API 33, and their AES-GCM behaviour has shifted across
 * releases; bundling BouncyCastle makes the algorithm set identical on every
 * supported device.
 *
 * The instance is passed directly rather than registered under a name. Android
 * ships its own cut-down provider already called "BC", so registering ours would
 * either clash with it or silently lose to it depending on install order — a
 * classic source of "works on my device" crypto bugs. Handing the object to
 * `CryptographyProvider.JDK` sidesteps the name lookup entirely.
 */
internal actual val relayCryptographyProvider: CryptographyProvider by lazy {
    CryptographyProvider.JDK(BouncyCastleProvider())
}
