package com.openbitfun.mobile.core.crypto

import com.openbitfun.mobile.core.protocol.EncryptedPayload
import com.openbitfun.mobile.core.protocol.RelayJson
import dev.whyoleg.cryptography.random.CryptographyRandom
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.SerializationException
import kotlinx.serialization.SerializationStrategy
import kotlinx.serialization.json.Json
import kotlin.random.Random

/**
 * The client half of the relay handshake, before a peer key is known.
 *
 * `RemoteCrypto.ets` models this as one object with a nullable shared key and a
 * `requireSharedKey()` that throws when it has not been derived yet. Splitting
 * it in two removes that state: a [RemoteCryptoSession] cannot exist without a
 * shared key, so no call site has to remember the ordering, and the failure
 * cannot reach runtime.
 */
public class RemoteHandshake internal constructor(
    private val keyPair: X25519KeyPair,
    private val cipher: RemoteCryptoCipher,
    private val random: Random,
    private val json: Json,
) {
    /** The value to send as the client public key in the pairing request. */
    public val publicKeyBase64: String = Base64Codec.encode(keyPair.publicKeyBytes)

    /**
     * Completes the handshake against the peer key from the pairing response.
     *
     * @throws RemoteCryptoException when the key is not decodable base64 or not
     * a valid X25519 public key.
     */
    public suspend fun accept(peerPublicKeyBase64: String): RemoteCryptoSession {
        val peerPublicKey = Base64Codec.decode(peerPublicKeyBase64, field = "peer public key")
        return RemoteCryptoSession(
            sharedKey = keyPair.sharedSecretWith(peerPublicKey),
            cipher = cipher,
            random = random,
            json = json,
        )
    }

    public companion object {
        /**
         * Starts a handshake with a fresh ephemeral key pair.
         *
         * [random] only supplies GCM nonces; the key pair comes from the
         * platform provider's own source and is not derived from it. Tests pass
         * a seeded [Random] to get reproducible nonces.
         */
        public suspend fun create(
            random: Random = CryptographyRandom.Default,
            json: Json = RelayJson,
        ): RemoteHandshake = RemoteHandshake(
            keyPair = X25519.generateKeyPair(),
            cipher = AesGcmCipher(),
            random = random,
            json = json,
        )
    }
}

/**
 * A paired session: everything here is encrypted under the X25519 shared secret.
 */
public class RemoteCryptoSession internal constructor(
    private val sharedKey: ByteArray,
    private val cipher: RemoteCryptoCipher,
    private val random: Random,
    private val json: Json,
) {
    /**
     * Serialises [value] and returns it as the relay's encrypted envelope.
     *
     * A fresh nonce is drawn per call. Reusing one under the same key would
     * expose the plaintext of both messages, so nonces are never derived from
     * message content or from a counter that could restart.
     */
    public suspend fun <T> encryptJson(
        serializer: SerializationStrategy<T>,
        value: T,
    ): EncryptedPayload {
        val plaintext = json.encodeToString(serializer, value).encodeToByteArray()
        val nonce = random.nextBytes(GCM_NONCE_SIZE)
        val encrypted = cipher.encrypt(plaintext, sharedKey, nonce)
        return EncryptedPayload(
            encryptedData = Base64Codec.encode(encrypted),
            nonce = Base64Codec.encode(nonce),
        )
    }

    /**
     * @throws RemoteCryptoException when the envelope is malformed, fails
     * authentication, or does not decode to [deserializer]'s shape.
     */
    public suspend fun <T> decryptJson(
        deserializer: DeserializationStrategy<T>,
        payload: EncryptedPayload,
    ): T {
        val encrypted = Base64Codec.decode(payload.encryptedData, field = "encrypted_data")
        val nonce = Base64Codec.decode(payload.nonce, field = "nonce")
        val plaintext = cipher.decrypt(encrypted, sharedKey, nonce).decodeToString()
        return try {
            json.decodeFromString(deserializer, plaintext)
        } catch (cause: SerializationException) {
            // The plaintext is authenticated at this point, so this is a shape
            // mismatch with the peer rather than tampering — but it is still
            // decrypted payload and must not appear in the message.
            throw RemoteCryptoException("Decrypted payload did not match the expected shape.", cause)
        }
    }
}
