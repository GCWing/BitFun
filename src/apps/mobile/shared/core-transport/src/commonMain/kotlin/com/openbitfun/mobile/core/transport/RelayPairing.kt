package com.openbitfun.mobile.core.transport

import com.openbitfun.mobile.core.crypto.RemoteCryptoException
import com.openbitfun.mobile.core.crypto.RemoteHandshake
import com.openbitfun.mobile.core.protocol.ChallengeCommand
import com.openbitfun.mobile.core.protocol.EncryptedPayload
import com.openbitfun.mobile.core.protocol.InitialSyncResponse
import com.openbitfun.mobile.core.protocol.PairChallengeResponse
import com.openbitfun.mobile.core.protocol.PairRequest
import com.openbitfun.mobile.core.protocol.RelayJson
import com.openbitfun.mobile.core.protocol.isError
import io.ktor.client.HttpClient

/**
 * Who this device claims to be when pairing.
 *
 * [password] is only supplied for rooms advertised with `auth=account`. It is a
 * credential: it goes into one encrypted command and is not retained here.
 */
public data class PairIdentity(
    val userId: String,
    val password: String? = null,
) {
    override fun toString(): String =
        "PairIdentity(userId=$userId, password=${if (password.isNullOrEmpty()) "absent" else "redacted"})"
}

/**
 * A room that has completed the handshake.
 *
 * The only way to obtain one is [RelayPairing.pair], so a [transport] always has
 * a live shared key behind it — the mobile client never holds a half-paired
 * room the way `RelayHttpClient`'s optional `crypto` field allows.
 */
public class PairedRoom internal constructor(
    public val descriptor: RelayDescriptor,
    public val initialSync: InitialSyncResponse,
    public val transport: RemoteCommandTransport,
)

/**
 * A pairing client over the platform's default engine — OkHttp on Android,
 * Darwin on iOS, both supplied by this module.
 *
 * This exists so callers never have to name [HttpClient]: Ktor is an
 * implementation detail of the transport, and a module that only wants to pair
 * should not have to depend on it to say so.
 */
public fun relayPairing(log: TransportLog = TransportLog.None): RelayPairing =
    RelayPairing(relayHttpClient(), log)

/**
 * The room pairing handshake, ported from `RelayHttpClient.pair`.
 *
 * Four steps, all under one shared key derived from the descriptor's public key:
 * publish our public key to `/pair`, decrypt the challenge, echo it back through
 * `/command`, and decrypt the initial sync that answers it.
 */
public class RelayPairing(
    private val httpClient: HttpClient,
    private val log: TransportLog = TransportLog.None,
) {
    /**
     * @param deviceId stable per install; also sent as `mobile_install_id`,
     * matching the HarmonyOS client, which passes the same value for both.
     * @param handshake defaults to a fresh ephemeral key pair; tests pass one
     * built over a seeded nonce source. It cannot be a default argument because
     * creating it suspends.
     * @throws RelayTransportException on any transport, crypto or peer-side
     * failure.
     */
    public suspend fun pair(
        descriptor: RelayDescriptor,
        deviceId: String,
        deviceName: String,
        identity: PairIdentity,
        handshake: RemoteHandshake? = null,
    ): PairedRoom {
        val keys = handshake ?: RemoteHandshake.create()
        val room = shortRoomId(descriptor.roomId)
        log.info("pair start room=$room")

        val endpoints = RelayEndpoints(httpClient, descriptor.relayUrl)
        val session = try {
            keys.accept(descriptor.publicKey)
        } catch (cause: RemoteCryptoException) {
            // A descriptor whose pk is not a usable X25519 key is a bad pairing
            // URL, not a relay problem — but the relay was never contacted, so
            // there is no status code to report.
            log.error("pair rejected room=$room reason=descriptor-key")
            throw RelayTransportException(RelayFailure.MalformedResponse, cause)
        }

        val challengePayload = endpoints.postForEncryptedPayload(
            path = pairPath(descriptor.roomId),
            body = RelayJson.encodeToString(
                PairRequest.serializer(),
                PairRequest(
                    publicKey = keys.publicKeyBase64,
                    deviceId = deviceId,
                    deviceName = deviceName,
                ),
            ),
            timeoutMs = RELAY_DEFAULT_TIMEOUT_MS,
        )
        val challenge = try {
            session.decryptJson(PairChallengeResponse.serializer(), challengePayload)
        } catch (cause: RemoteCryptoException) {
            log.error("pair challenge undecryptable room=$room")
            throw RelayTransportException(RelayFailure.MalformedResponse, cause)
        }
        log.info("pair challenge received room=$room")

        val transport = RoomRemoteCommandTransport(
            endpoints = endpoints,
            roomId = descriptor.roomId,
            session = session,
            log = log,
        )

        // The echo goes out as a raw encrypted body rather than through the
        // command transport: at this point the peer has not yet accepted us, so
        // the reply is an initial sync rather than a command result.
        val challengeCommand = ChallengeCommand(
            challengeEcho = challenge.challenge,
            deviceId = deviceId,
            deviceName = deviceName,
            mobileInstallId = deviceId,
            userId = identity.userId.trim(),
            password = identity.password?.takeIf { it.isNotEmpty() },
        )
        val encrypted = session.encryptJson(ChallengeCommand.serializer(), challengeCommand)
        val syncPayload = endpoints.postForEncryptedPayload(
            path = commandPath(descriptor.roomId),
            body = RelayJson.encodeToString(EncryptedPayload.serializer(), encrypted),
            timeoutMs = RELAY_DEFAULT_TIMEOUT_MS,
        )
        val initialSync = try {
            session.decryptJson(InitialSyncResponse.serializer(), syncPayload)
        } catch (cause: RemoteCryptoException) {
            log.error("pair sync undecryptable room=$room")
            throw RelayTransportException(RelayFailure.MalformedResponse, cause)
        }

        if (initialSync.isError) {
            log.warn("pair rejected room=$room")
            throw RelayTransportException(RelayFailure.RemoteRejected(initialSync.message))
        }
        log.info("pair complete room=$room")

        return PairedRoom(descriptor, initialSync, transport)
    }
}
