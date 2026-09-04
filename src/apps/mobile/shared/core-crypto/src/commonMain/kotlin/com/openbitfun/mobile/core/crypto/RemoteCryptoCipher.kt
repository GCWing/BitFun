package com.openbitfun.mobile.core.crypto

import dev.whyoleg.cryptography.BinarySize.Companion.bits
import dev.whyoleg.cryptography.DelicateCryptographyApi
import dev.whyoleg.cryptography.algorithms.AES

/** Bytes of authentication tag appended to every ciphertext. */
internal const val GCM_TAG_SIZE: Int = 16

/** Bytes of nonce the relay protocol uses for AES-GCM. */
internal const val GCM_NONCE_SIZE: Int = 12

/**
 * The seam between the relay envelope and the actual AES-GCM implementation,
 * carried over from `RemoteCrypto.ets` so tests can drive the envelope logic
 * without a real cipher.
 *
 * Implementations return, and accept, ciphertext with the 16-byte tag appended
 * — the layout the desktop peer produces and the HarmonyOS client already
 * parses as `slice(0, len - 16)` plus a trailing tag.
 */
internal interface RemoteCryptoCipher {
    suspend fun encrypt(plaintext: ByteArray, key: ByteArray, nonce: ByteArray): ByteArray

    suspend fun decrypt(ciphertext: ByteArray, key: ByteArray, nonce: ByteArray): ByteArray
}

/**
 * AES-256-GCM with a 128-bit tag and no associated data, matching the HarmonyOS
 * `GcmParamsSpec` that passes an empty `aad`.
 */
internal class AesGcmCipher(
    private val algorithm: AES.GCM = relayCryptographyProvider.get(AES.GCM),
) : RemoteCryptoCipher {
    // `encryptWithIv` is delicate because a caller-chosen nonce repeated under
    // one key destroys GCM's confidentiality. We take it because the nonce
    // travels beside the ciphertext in the envelope, so the library cannot
    // generate it for us. The safety condition is met upstream: the only caller
    // is RemoteCryptoSession.encryptJson, which draws a fresh random nonce per
    // message and never derives one from content or from a resettable counter.
    @OptIn(DelicateCryptographyApi::class)
    override suspend fun encrypt(plaintext: ByteArray, key: ByteArray, nonce: ByteArray): ByteArray {
        val cipher = cipherFor(key, nonce)
        return cipher.encryptWithIv(nonce, plaintext)
    }

    @OptIn(DelicateCryptographyApi::class)
    override suspend fun decrypt(ciphertext: ByteArray, key: ByteArray, nonce: ByteArray): ByteArray {
        if (ciphertext.size < GCM_TAG_SIZE) {
            // Without this the provider reports a generic authentication
            // failure, which reads as "someone tampered with the message" when
            // the real cause is a truncated payload.
            throw RemoteCryptoException(
                "Encrypted payload is ${ciphertext.size} bytes, shorter than the " +
                    "$GCM_TAG_SIZE-byte tag.",
            )
        }
        val cipher = cipherFor(key, nonce)
        return try {
            cipher.decryptWithIv(nonce, ciphertext)
        } catch (cause: Throwable) {
            throw RemoteCryptoException("Encrypted payload failed authentication.", cause)
        }
    }

    private suspend fun cipherFor(key: ByteArray, nonce: ByteArray) = run {
        if (key.size != AES_256_KEY_SIZE) {
            throw RemoteCryptoException("AES key must be $AES_256_KEY_SIZE bytes, got ${key.size}.")
        }
        if (nonce.size != GCM_NONCE_SIZE) {
            throw RemoteCryptoException("Nonce must be $GCM_NONCE_SIZE bytes, got ${nonce.size}.")
        }
        algorithm.keyDecoder()
            .decodeFromByteArray(AES.Key.Format.RAW, key)
            .cipher(tagSize = 128.bits)
    }

    private companion object {
        const val AES_256_KEY_SIZE = 32
    }
}

/** AES-GCM operations used by the cloud account envelope and master-key wrapper. */
public object CloudAccountCipher {
    public suspend fun encrypt(plaintext: ByteArray, key: ByteArray, nonce: ByteArray): ByteArray {
        require(key.size == 32) { "Cloud account key must be 32 bytes." }
        require(nonce.size == 12) { "Cloud account nonce must be 12 bytes." }
        return AesGcmCipher().encrypt(plaintext, key, nonce)
    }

    public suspend fun decrypt(ciphertext: ByteArray, key: ByteArray, nonce: ByteArray): ByteArray {
        require(key.size == 32) { "Cloud account key must be 32 bytes." }
        require(nonce.size == 12) { "Cloud account nonce must be 12 bytes." }
        return AesGcmCipher().decrypt(ciphertext, key, nonce)
    }
}
