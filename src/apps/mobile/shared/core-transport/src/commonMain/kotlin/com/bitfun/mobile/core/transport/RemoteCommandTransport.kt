package com.bitfun.mobile.core.transport

import com.bitfun.mobile.core.crypto.RemoteCryptoException
import com.bitfun.mobile.core.crypto.RemoteCryptoSession
import com.bitfun.mobile.core.protocol.CommandStatus
import com.bitfun.mobile.core.protocol.EncryptedPayload
import com.bitfun.mobile.core.protocol.RelayJson
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.protocol.isError
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.serializer

/**
 * Sends one command to the paired desktop and returns its reply.
 *
 * Ported from `RemoteCommandTransport` in `services/RemoteCommandTransport.ets`,
 * minus its `reset()`. That method exists there because `RelayHttpClient` is a
 * long-lived singleton rebound on every pairing, so it has to be emptied when a
 * session ends. Here a transport is created by a successful pairing and owns an
 * immutable session, so ending a session means dropping the object — there is
 * no state left to clear, and no window in which a transport exists without a
 * key.
 */
public interface RemoteCommandTransport {
    /**
     * @throws RelayTransportException on any transport, decoding or peer-side
     * failure; [RelayFailure.RemoteRejected] carries the desktop's own message.
     */
    public suspend fun <T : CommandStatus> send(
        deserializer: DeserializationStrategy<T>,
        command: RemoteCommand,
        timeoutMs: Long = RELAY_DEFAULT_TIMEOUT_MS,
    ): T
}

/** Reified convenience over [RemoteCommandTransport.send]. */
public suspend inline fun <reified T : CommandStatus> RemoteCommandTransport.send(
    command: RemoteCommand,
    timeoutMs: Long = RELAY_DEFAULT_TIMEOUT_MS,
): T = send(serializer(), command, timeoutMs)

/**
 * Command transport over a paired room, i.e. `POST /api/rooms/{roomId}/command`
 * with an AES-GCM envelope in both directions.
 */
public class RoomRemoteCommandTransport internal constructor(
    private val endpoints: RelayEndpoints,
    private val roomId: String,
    private val session: RemoteCryptoSession,
    private val log: TransportLog,
) : RemoteCommandTransport {
    override suspend fun <T : CommandStatus> send(
        deserializer: DeserializationStrategy<T>,
        command: RemoteCommand,
        timeoutMs: Long,
    ): T {
        val label = "cmd=${command.cmd} request=${shortRequestId(command.requestId.orEmpty())} " +
            "room=${shortRoomId(roomId)}"
        log.info("command start $label")

        val encrypted = try {
            session.encryptJson(RemoteCommand.serializer(), command)
        } catch (cause: RemoteCryptoException) {
            log.error("command encrypt failed $label")
            throw RelayTransportException(RelayFailure.MalformedResponse, cause)
        }

        val responsePayload = endpoints.postForEncryptedPayload(
            path = commandPath(roomId),
            body = RelayJson.encodeToString(EncryptedPayload.serializer(), encrypted),
            timeoutMs = timeoutMs,
        )

        val response = try {
            session.decryptJson(deserializer, responsePayload)
        } catch (cause: RemoteCryptoException) {
            log.error("command decrypt failed $label")
            throw RelayTransportException(RelayFailure.MalformedResponse, cause)
        }

        if (response.isError) {
            // The message is the desktop's, already localized by the desktop.
            // It is echoed to the user but never used for control flow.
            log.warn("command rejected $label")
            throw RelayTransportException(RelayFailure.RemoteRejected(response.message))
        }
        log.info("command done $label resp=${response.resp ?: "unknown"}")
        return response
    }
}

internal fun commandPath(roomId: String): String = "/api/rooms/$roomId/command"

internal fun pairPath(roomId: String): String = "/api/rooms/$roomId/pair"
