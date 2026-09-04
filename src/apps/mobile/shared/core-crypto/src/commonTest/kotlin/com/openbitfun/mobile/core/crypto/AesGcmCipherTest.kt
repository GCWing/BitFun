package com.openbitfun.mobile.core.crypto

import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

/**
 * The known-answer cases are test cases 13 and 14 from the GCM specification
 * (McGrew & Viega), the all-zero AES-256 pair.
 *
 * They pin more than the algorithm. Case 13 has an empty plaintext, so its whole
 * output is the tag — decrypting those 16 bytes back to nothing proves the tag
 * is *appended* and is 16 bytes long. That layout is the interop contract with
 * the desktop peer and with the HarmonyOS client, which splits ciphertext as
 * `slice(0, len - 16)`; a provider that prepended the tag or used a shorter one
 * would still round-trip against itself and fail only against a real peer.
 *
 * Note that exactly one test encrypts under the all-zero key and nonce. The JDK
 * refuses to encrypt twice with the same GCM key/IV pair — correctly, since that
 * would leak both plaintexts — and the provider pools its cipher instances, so a
 * second zero-pair encryption anywhere in this class fails with
 * `InvalidAlgorithmParameterException`. Every other test picks its own nonce.
 */
class AesGcmCipherTest {
    private val cipher = AesGcmCipher()
    private val zeroKey = ByteArray(32)
    private val zeroNonce = ByteArray(GCM_NONCE_SIZE)

    private fun nonce(seed: Byte) = ByteArray(GCM_NONCE_SIZE) { seed }

    @Test
    fun knownPlaintextEncryptsToTheKnownCiphertextAndTag() = runTest {
        val output = cipher.encrypt(ByteArray(16), zeroKey, zeroNonce)
        assertEquals(
            "cea7403d4d606b6e074ec5d3baf39d18" + "d0d1c8a799996bf0265b98b5d48ab919",
            output.toHex(),
        )
    }

    @Test
    fun theKnownCiphertextDecryptsBackToItsPlaintext() = runTest {
        val ciphertext =
            ("cea7403d4d606b6e074ec5d3baf39d18" + "d0d1c8a799996bf0265b98b5d48ab919").hexToBytes()
        assertEquals(ByteArray(16).toHex(), cipher.decrypt(ciphertext, zeroKey, zeroNonce).toHex())
    }

    @Test
    fun anEmptyMessageIsCarriedByItsTagAlone() = runTest {
        val tagOnly = "530f8afbc74536b9a963b4f1c4cb738b".hexToBytes()
        assertEquals(GCM_TAG_SIZE, tagOnly.size)
        assertEquals(0, cipher.decrypt(tagOnly, zeroKey, zeroNonce).size)
    }

    @Test
    fun ciphertextGrowsByExactlyTheTagLength() = runTest {
        val plaintext = "hello relay".encodeToByteArray()
        val output = cipher.encrypt(plaintext, zeroKey, nonce(1))
        assertEquals(plaintext.size + GCM_TAG_SIZE, output.size)
    }

    @Test
    fun aTamperedTagFailsAuthentication() = runTest {
        val output = cipher.encrypt("hello relay".encodeToByteArray(), zeroKey, nonce(2))
        output[output.lastIndex] = (output[output.lastIndex].toInt() xor 0x01).toByte()

        assertFailsWith<RemoteCryptoException> { cipher.decrypt(output, zeroKey, nonce(2)) }
    }

    @Test
    fun theWrongNonceFailsAuthentication() = runTest {
        val output = cipher.encrypt("hello relay".encodeToByteArray(), zeroKey, nonce(3))

        assertFailsWith<RemoteCryptoException> { cipher.decrypt(output, zeroKey, nonce(4)) }
    }

    @Test
    fun aTruncatedPayloadReportsItsLengthRatherThanTampering() = runTest {
        val failure = assertFailsWith<RemoteCryptoException> {
            cipher.decrypt(ByteArray(8), zeroKey, zeroNonce)
        }
        assertTrue("8 bytes" in failure.message.orEmpty())
    }

    @Test
    fun rejectsKeysAndNoncesOfTheWrongSize() = runTest {
        assertFailsWith<RemoteCryptoException> {
            cipher.encrypt(ByteArray(1), ByteArray(16), nonce(5))
        }
        assertFailsWith<RemoteCryptoException> {
            cipher.encrypt(ByteArray(1), zeroKey, ByteArray(16))
        }
    }
}
