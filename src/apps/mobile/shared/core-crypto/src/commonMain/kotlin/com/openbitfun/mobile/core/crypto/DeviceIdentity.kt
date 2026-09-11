package com.openbitfun.mobile.core.crypto

import dev.whyoleg.cryptography.algorithms.HKDF
import dev.whyoleg.cryptography.algorithms.SHA256
import dev.whyoleg.cryptography.BinarySize.Companion.bytes
import dev.whyoleg.cryptography.random.CryptographyRandom

/** Private device identity; only its public key is registered with the relay. */
public object DeviceIdentity {
    public fun randomBytes(size: Int): ByteArray = CryptographyRandom.Default.nextBytes(size)

    public fun generateSecret(): ByteArray = CryptographyRandom.Default.nextBytes(32)

    public suspend fun publicKey(secret: ByteArray): ByteArray =
        X25519.fromPrivateKeyBytes(secret).publicKeyBytes

    public suspend fun messageKey(secret: ByteArray, peerPublic: ByteArray): ByteArray {
        val pair = X25519.fromPrivateKeyBytes(secret)
        val shared = pair.sharedSecretWith(peerPublic)
        require(shared.any { it != 0.toByte() }) { "Invalid peer public key." }
        val local = pair.publicKeyBytes
        val order = local.indices.firstOrNull { local[it] != peerPublic[it] }
        val localFirst = order == null || (local[order].toInt() and 255) < (peerPublic[order].toInt() and 255)
        val info = if (localFirst) local + peerPublic else peerPublic + local
        return try {
            relayCryptographyProvider.get(HKDF).secretDerivation(
                SHA256, 32.bytes, "OpenBitFun Relay v1.0.0 device key".encodeToByteArray(), info,
            ).deriveSecretToByteArray(shared)
        } finally { shared.fill(0) }
    }
}
