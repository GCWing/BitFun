package com.openbitfun.mobile.core.transport

import com.openbitfun.mobile.core.crypto.RemoteCryptoSession
import com.openbitfun.mobile.core.crypto.RemoteHandshake
import com.openbitfun.mobile.core.protocol.EncryptedPayload
import com.openbitfun.mobile.core.protocol.PairRequest
import com.openbitfun.mobile.core.protocol.RelayJson
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.SerializationStrategy
import kotlinx.serialization.json.JsonObject

/**
 * The desktop half of the handshake, standing in for the peer that
 * `harmonyos/tools/fake-relay.mjs` emulates.
 *
 * X25519 is symmetric, so the peer runs the same [RemoteHandshake] the client
 * does: whichever side calls `accept` with the other's public key derives the
 * same shared secret. That means these tests exercise the real cipher rather
 * than a stub — a change that broke the envelope layout would fail here, not
 * only against a live desktop.
 */
internal class DesktopPeer private constructor(private val handshake: RemoteHandshake) {
    val publicKeyBase64: String get() = handshake.publicKeyBase64

    private var session: RemoteCryptoSession? = null

    /** Derives the shared key from the client's `/pair` body, as the desktop does. */
    suspend fun acceptPairRequest(body: String): PairRequest {
        val request = RelayJson.decodeFromString(PairRequest.serializer(), body)
        session = handshake.accept(request.publicKey)
        return request
    }

    suspend fun <T> encrypt(serializer: SerializationStrategy<T>, value: T): String =
        RelayJson.encodeToString(
            EncryptedPayload.serializer(),
            requireSession().encryptJson(serializer, value),
        )

    suspend fun <T> decrypt(deserializer: DeserializationStrategy<T>, body: String): T =
        requireSession().decryptJson(
            deserializer,
            RelayJson.decodeFromString(EncryptedPayload.serializer(), body),
        )

    /** Decrypts without imposing a shape, so tests can assert which keys are on the wire. */
    suspend fun decryptRaw(body: String): JsonObject = decrypt(JsonObject.serializer(), body)

    companion object {
        /** 32 lowercase hex characters, the shape `pairing.rs` validates the echo against. */
        const val CHALLENGE: String = "9f2c0a17be4d5386a10c7f43de99b025"

        suspend fun create(): DesktopPeer = DesktopPeer(RemoteHandshake.create())
    }

    private fun requireSession(): RemoteCryptoSession =
        session ?: error("DesktopPeer was used before /pair derived a shared key")
}

/** Collects transport log lines so tests can assert what does *not* appear in them. */
internal class RecordingLog : TransportLog {
    val lines: MutableList<String> = mutableListOf()

    override fun info(message: String) {
        lines += message
    }

    override fun warn(message: String) {
        lines += message
    }

    override fun error(message: String) {
        lines += message
    }
}
