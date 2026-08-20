package com.bitfun.mobile.core.crypto

import dev.whyoleg.cryptography.algorithms.XDH

/** Length of an X25519 public key and of a raw private scalar, in bytes. */
internal const val X25519_KEY_SIZE: Int = 32

/**
 * One side of the pairing handshake.
 *
 * The private key stays inside the provider — there is no accessor that returns
 * it as bytes. Only [publicKeyBytes] leaves this object, which is exactly what
 * the handshake needs to publish.
 */
internal class X25519KeyPair(
    private val privateKey: XDH.PrivateKey,
    val publicKeyBytes: ByteArray,
) {
    /**
     * The raw X25519 shared secret with [peerPublicKey].
     *
     * No KDF is applied: the desktop peer feeds this straight into AES-256, so
     * hashing it here would simply make the two sides disagree. It is a property
     * of the existing wire protocol, not a recommendation for a new one.
     */
    suspend fun sharedSecretWith(peerPublicKey: ByteArray): ByteArray {
        if (peerPublicKey.size != X25519_KEY_SIZE) {
            throw RemoteCryptoException(
                "Peer public key must be $X25519_KEY_SIZE bytes, got ${peerPublicKey.size}.",
            )
        }
        val decoded = try {
            X25519.publicKeyDecoder(XDH.Curve.X25519)
                .decodeFromByteArray(XDH.PublicKey.Format.RAW, peerPublicKey)
        } catch (cause: Throwable) {
            throw RemoteCryptoException("Peer public key is not a valid X25519 point.", cause)
        }
        return privateKey.sharedSecretGenerator().generateSharedSecretToByteArray(decoded)
    }
}

internal object X25519 {
    private val algorithm: XDH get() = relayCryptographyProvider.get(XDH)

    fun publicKeyDecoder(curve: XDH.Curve) = algorithm.publicKeyDecoder(curve)

    suspend fun generateKeyPair(): X25519KeyPair {
        val pair = algorithm.keyPairGenerator(XDH.Curve.X25519).generateKey()
        return X25519KeyPair(
            privateKey = pair.privateKey,
            publicKeyBytes = pair.publicKey.encodeToByteArray(XDH.PublicKey.Format.RAW),
        )
    }

    /**
     * Rebuilds a key pair from a raw 32-byte scalar.
     *
     * Only the tests use this — it is what lets the RFC 7748 vectors run against
     * a fixed private key instead of a generated one. Nothing in the pairing
     * flow persists or reimports a private key.
     */
    suspend fun fromPrivateKeyBytes(privateKeyBytes: ByteArray): X25519KeyPair {
        require(privateKeyBytes.size == X25519_KEY_SIZE) {
            "Private scalar must be $X25519_KEY_SIZE bytes."
        }
        val privateKey = algorithm.privateKeyDecoder(XDH.Curve.X25519)
            .decodeFromByteArray(XDH.PrivateKey.Format.RAW, privateKeyBytes)
        return X25519KeyPair(
            privateKey = privateKey,
            publicKeyBytes = privateKey.getPublicKey().encodeToByteArray(XDH.PublicKey.Format.RAW),
        )
    }
}
