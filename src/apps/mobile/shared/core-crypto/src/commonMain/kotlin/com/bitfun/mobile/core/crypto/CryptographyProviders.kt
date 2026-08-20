package com.bitfun.mobile.core.crypto

import dev.whyoleg.cryptography.CryptographyProvider

/**
 * The provider every algorithm in this module is resolved from.
 *
 * This is chosen per platform rather than left to `CryptographyProvider.Default`
 * on purpose: the default picks whichever provider happens to be on the
 * classpath first, and on Android that is a JDK provider whose XDH support only
 * exists from API 33. Pairing would then work on new phones and fail on the
 * minSdk 26 floor we support — the worst kind of failure to discover late.
 *
 * `cryptography-provider-optimal` has the same problem and is not used anywhere
 * in this module.
 */
internal expect val relayCryptographyProvider: CryptographyProvider
