package com.openbitfun.mobile.core.feature.pairing

import com.openbitfun.mobile.core.crypto.RemoteCryptoSession
import com.openbitfun.mobile.core.crypto.RemoteHandshake
import com.openbitfun.mobile.core.protocol.EncryptedPayload
import com.openbitfun.mobile.core.protocol.InitialSyncResponse
import com.openbitfun.mobile.core.protocol.PairChallengeResponse
import com.openbitfun.mobile.core.protocol.PairRequest
import com.openbitfun.mobile.core.protocol.RelayJson
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.engine.mock.toByteArray
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import kotlinx.serialization.SerializationStrategy

internal const val ROOM_ID: String = "0123456789abcdef0123456789abcdef"
internal const val RELAY_URL: String = "https://relay.example.com"

/**
 * The desktop half of the handshake, running the real cipher.
 *
 * A trimmed sibling of `:core-transport`'s `DesktopPeer` — that one lives in
 * another module's test source set, which is not on this module's test
 * classpath. It is kept small on purpose: what is under test here is the state
 * machine, and the envelope itself is already covered next door.
 */
internal class FakeDesktop private constructor(private val handshake: RemoteHandshake) {
    private var session: RemoteCryptoSession? = null

    /**
     * Flip to make every later `/command` fail at the relay.
     *
     * Which command it was does not matter to what is under test here: a
     * heartbeat's only question is whether the room still answers, and the
     * transport already turns a 503 into the failure the store reads.
     */
    var offline: Boolean = false

    val publicKeyBase64: String get() = handshake.publicKeyBase64

    private suspend fun <T> encrypt(serializer: SerializationStrategy<T>, value: T): String =
        RelayJson.encodeToString(
            EncryptedPayload.serializer(),
            requireNotNull(session).encryptJson(serializer, value),
        )

    /** A relay that completes the handshake and answers `/command` with [sync]. */
    fun engine(sync: InitialSyncResponse): MockEngine = MockEngine { request ->
        val body = request.body.toByteArray().decodeToString()
        when (request.url.encodedPath) {
            "/api/rooms/$ROOM_ID/pair" -> {
                session = handshake.accept(
                    RelayJson.decodeFromString(PairRequest.serializer(), body).publicKey,
                )
                json(
                    encrypt(
                        PairChallengeResponse.serializer(),
                        PairChallengeResponse(CHALLENGE, timestamp = 1_770_000_000),
                    ),
                )
            }

            "/api/rooms/$ROOM_ID/command" ->
                if (offline) {
                    respond("", HttpStatusCode.ServiceUnavailable)
                } else {
                    // A ping only reads `resp`, so the sync doubles as its reply.
                    json(encrypt(InitialSyncResponse.serializer(), sync))
                }

            else -> respond("", HttpStatusCode.NotFound)
        }
    }

    private fun io.ktor.client.engine.mock.MockRequestHandleScope.json(body: String) =
        respond(body, HttpStatusCode.OK, headersOf(HttpHeaders.ContentType, "application/json"))

    companion object {
        /** 32 lowercase hex characters, the shape `pairing.rs` validates the echo against. */
        const val CHALLENGE: String = "9f2c0a17be4d5386a10c7f43de99b025"

        suspend fun create(): FakeDesktop = FakeDesktop(RemoteHandshake.create())
    }
}

/** Collects log lines so tests can assert what does *not* appear in them. */
internal class RecordingCoreLog : com.openbitfun.mobile.core.feature.CoreLog {
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
