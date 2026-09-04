package com.openbitfun.mobile.core.crypto

import dev.whyoleg.cryptography.CryptographyProvider
import dev.whyoleg.cryptography.providers.cryptokit.CryptoKit

/**
 * iOS uses CryptoKit, which covers both X25519 and AES-GCM natively from iOS 13
 * and needs no bundled implementation.
 */
internal actual val relayCryptographyProvider: CryptographyProvider by lazy {
    CryptographyProvider.CryptoKit
}
