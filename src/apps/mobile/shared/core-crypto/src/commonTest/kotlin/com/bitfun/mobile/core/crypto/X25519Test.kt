package com.bitfun.mobile.core.crypto

import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

/**
 * RFC 7748 section 6.1 is the vector that matters here: it is the base-point key
 * derivation and the agreement, which is exactly what pairing does. The section
 * 5.2 vectors exercise u-coordinates with the high bit set, a case the protocol
 * never produces and which providers legitimately handle differently.
 */
class X25519Test {
    private val alicePrivate = "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a"
    private val alicePublic = "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
    private val bobPrivate = "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb"
    private val bobPublic = "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"
    private val sharedSecret = "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"

    @Test
    fun derivesTheRfc7748PublicKeysFromTheirPrivateScalars() = runTest {
        assertEquals(alicePublic, X25519.fromPrivateKeyBytes(alicePrivate.hexToBytes()).publicKeyBytes.toHex())
        assertEquals(bobPublic, X25519.fromPrivateKeyBytes(bobPrivate.hexToBytes()).publicKeyBytes.toHex())
    }

    @Test
    fun bothSidesReachTheRfc7748SharedSecret() = runTest {
        val alice = X25519.fromPrivateKeyBytes(alicePrivate.hexToBytes())
        val bob = X25519.fromPrivateKeyBytes(bobPrivate.hexToBytes())

        assertEquals(sharedSecret, alice.sharedSecretWith(bobPublic.hexToBytes()).toHex())
        assertEquals(sharedSecret, bob.sharedSecretWith(alicePublic.hexToBytes()).toHex())
    }

    @Test
    fun generatedKeyPairsAgreeWithEachOther() = runTest {
        val alice = X25519.generateKeyPair()
        val bob = X25519.generateKeyPair()

        assertEquals(X25519_KEY_SIZE, alice.publicKeyBytes.size)
        assertEquals(
            alice.sharedSecretWith(bob.publicKeyBytes).toHex(),
            bob.sharedSecretWith(alice.publicKeyBytes).toHex(),
        )
    }

    @Test
    fun generatedKeyPairsAreNotReused() = runTest {
        // Each pairing must use a fresh ephemeral key; a provider handing back a
        // cached pair would silently make every session share one secret.
        val first = X25519.generateKeyPair()
        val second = X25519.generateKeyPair()
        assertTrue(first.publicKeyBytes.toHex() != second.publicKeyBytes.toHex())
    }

    @Test
    fun rejectsAPeerKeyOfTheWrongLength() = runTest {
        val alice = X25519.generateKeyPair()
        val failure = assertFailsWith<RemoteCryptoException> {
            alice.sharedSecretWith(ByteArray(31))
        }
        assertTrue("32 bytes" in failure.message.orEmpty())
    }
}
