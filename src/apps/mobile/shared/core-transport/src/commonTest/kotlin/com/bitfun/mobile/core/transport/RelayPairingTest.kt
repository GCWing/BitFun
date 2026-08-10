package com.bitfun.mobile.core.transport

import com.bitfun.mobile.core.protocol.ChallengeCommand
import com.bitfun.mobile.core.protocol.InitialSyncResponse
import com.bitfun.mobile.core.protocol.PairChallengeResponse
import com.bitfun.mobile.core.protocol.SessionListResponse
import com.bitfun.mobile.core.protocol.RemoteCommand
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.engine.mock.toByteArray
import io.ktor.client.request.HttpRequestData
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

private const val ROOM_ID = "0123456789abcdef0123456789abcdef"
private const val RELAY_URL = "https://relay.example.com"
private const val DEVICE_ID = "android-install-42"
private const val DEVICE_NAME = "Pixel"

/**
 * Exercises the full four-step handshake against a [DesktopPeer] speaking the
 * real cipher, which is the JVM-side equivalent of pointing the client at
 * `harmonyos/tools/fake-relay.mjs`.
 */
class RelayPairingTest {
    @Test
    fun completesTheHandshakeAndReturnsInitialSync() = runTest {
        val peer = DesktopPeer.create()
        var echoed: ChallengeCommand? = null

        val engine = MockEngine { request ->
            when (request.path()) {
                "/api/rooms/$ROOM_ID/pair" -> {
                    peer.acceptPairRequest(request.text())
                    jsonResponse(
                        peer.encrypt(
                            PairChallengeResponse.serializer(),
                            PairChallengeResponse(DesktopPeer.CHALLENGE, timestamp = 1_770_000_000),
                        ),
                    )
                }

                "/api/rooms/$ROOM_ID/command" -> {
                    echoed = peer.decrypt(ChallengeCommand.serializer(), request.text())
                    jsonResponse(
                        peer.encrypt(
                            InitialSyncResponse.serializer(),
                            InitialSyncResponse(
                                resp = "ok",
                                hasWorkspace = true,
                                projectName = "BitFun",
                                authenticatedUserId = "alice",
                            ),
                        ),
                    )
                }

                else -> respond("", HttpStatusCode.NotFound)
            }
        }

        val paired = pairWith(peer, engine, PairIdentity(userId = "alice"))

        assertEquals(DesktopPeer.CHALLENGE, echoed?.challengeEcho)
        assertEquals(DEVICE_ID, echoed?.deviceId)
        // The HarmonyOS client sends the install id under both names; the desktop
        // reads mobile_install_id when de-duplicating devices.
        assertEquals(DEVICE_ID, echoed?.mobileInstallId)
        assertEquals("alice", echoed?.userId)
        assertEquals("BitFun", paired.initialSync.projectName)
        assertEquals("alice", paired.initialSync.authenticatedUserId)
        assertEquals(
            listOf("/api/rooms/$ROOM_ID/pair", "/api/rooms/$ROOM_ID/command"),
            engine.requestHistory.map { it.url.encodedPath },
        )
    }

    @Test
    fun sendsNoPasswordFieldForAPasswordlessRoom() = runTest {
        val peer = DesktopPeer.create()
        var commandKeys: Set<String> = emptySet()

        val engine = handshakeEngine(peer) { body ->
            commandKeys = peer.decryptRaw(body).keys
            peer.encrypt(InitialSyncResponse.serializer(), InitialSyncResponse(resp = "ok"))
        }

        pairWith(peer, engine, PairIdentity(userId = "alice"))

        // explicitNulls = false must hold end to end: the desktop distinguishes
        // an absent password from a null one when deciding whether to check it.
        assertFalse("password" in commandKeys)
        assertContains(commandKeys, "challenge_echo")
    }

    @Test
    fun sendsThePasswordWhenTheRoomRequiresAccountAuth() = runTest {
        val peer = DesktopPeer.create()
        var command: ChallengeCommand? = null

        val engine = handshakeEngine(peer) { body ->
            command = peer.decrypt(ChallengeCommand.serializer(), body)
            peer.encrypt(InitialSyncResponse.serializer(), InitialSyncResponse(resp = "ok"))
        }

        pairWith(peer, engine, PairIdentity(userId = "alice", password = "s3cret"))

        assertEquals("s3cret", command?.password)
        // An empty password means "no password", not "the empty password".
        assertFalse("s3cret" in command.toString())
    }

    @Test
    fun anEmptyPasswordIsTreatedAsAbsent() = runTest {
        val peer = DesktopPeer.create()
        var commandKeys: Set<String> = emptySet()

        val engine = handshakeEngine(peer) { body ->
            commandKeys = peer.decryptRaw(body).keys
            peer.encrypt(InitialSyncResponse.serializer(), InitialSyncResponse(resp = "ok"))
        }

        pairWith(peer, engine, PairIdentity(userId = "alice", password = ""))

        assertFalse("password" in commandKeys)
    }

    @Test
    fun trimsTheUserIdBeforeSendingIt() = runTest {
        val peer = DesktopPeer.create()
        var command: ChallengeCommand? = null

        val engine = handshakeEngine(peer) { body ->
            command = peer.decrypt(ChallengeCommand.serializer(), body)
            peer.encrypt(InitialSyncResponse.serializer(), InitialSyncResponse(resp = "ok"))
        }

        pairWith(peer, engine, PairIdentity(userId = "  alice  "))

        assertEquals("alice", command?.userId)
    }

    @Test
    fun aDesktopRejectionBecomesRemoteRejected() = runTest {
        val peer = DesktopPeer.create()
        val engine = handshakeEngine(peer) {
            peer.encrypt(
                InitialSyncResponse.serializer(),
                InitialSyncResponse(
                    resp = "error",
                    message = "This remote URL is already protected by a different user ID.",
                ),
            )
        }

        val failure = assertFailsWith<RelayTransportException> {
            pairWith(peer, engine, PairIdentity(userId = "mallory"))
        }

        assertEquals(
            RelayFailure.RemoteRejected(
                "This remote URL is already protected by a different user ID.",
            ),
            failure.failure,
        )
    }

    @Test
    fun aDescriptorKeyThatIsNotAValidPointFailsBeforeAnyRequest() = runTest {
        val engine = MockEngine { error("the transport must not reach the relay") }
        val pairing = RelayPairing(relayHttpClient(engine))

        val failure = assertFailsWith<RelayTransportException> {
            pairing.pair(
                descriptor = descriptorFor("not base64 at all!!"),
                deviceId = DEVICE_ID,
                deviceName = DEVICE_NAME,
                identity = PairIdentity(userId = "alice"),
            )
        }

        assertEquals(RelayFailure.MalformedResponse, failure.failure)
        assertEquals(0, engine.requestHistory.size)
    }

    /**
     * A relay that answers 200 with something other than the envelope — a login
     * page from a captive portal, say — must not surface as a crypto failure.
     */
    @Test
    fun aNonEnvelopeBodyIsAMalformedResponse() = runTest {
        val peer = DesktopPeer.create()
        val engine = MockEngine { respond("<html>captive portal</html>", HttpStatusCode.OK) }

        val failure = assertFailsWith<RelayTransportException> {
            pairWith(peer, engine, PairIdentity(userId = "alice"))
        }

        assertEquals(RelayFailure.MalformedResponse, failure.failure)
    }

    @Test
    fun logsCarryTruncatedRoomIdsOnly() = runTest {
        val peer = DesktopPeer.create()
        val log = RecordingLog()
        val engine = handshakeEngine(peer) {
            peer.encrypt(InitialSyncResponse.serializer(), InitialSyncResponse(resp = "ok"))
        }

        pairWith(peer, engine, PairIdentity(userId = "alice"), log = log)

        assertTrue(log.lines.isNotEmpty())
        val joined = log.lines.joinToString("\n")
        assertFalse(ROOM_ID in joined, "the full room id reached the log")
        assertContains(joined, "room=01234567")
    }

    @Test
    fun mapsHttpStatusCodesToTypedFailures() = runTest {
        val expected = mapOf(
            401 to RelayFailure.PairRejected,
            403 to RelayFailure.PairRejected,
            404 to RelayFailure.RoomNotFound,
            408 to RelayFailure.Timeout,
            418 to RelayFailure.UnexpectedStatus(418),
            429 to RelayFailure.RateLimited,
            500 to RelayFailure.RelayUnavailable(500),
            502 to RelayFailure.RelayUnavailable(502),
            504 to RelayFailure.Timeout,
        )

        for ((status, failure) in expected) {
            val peer = DesktopPeer.create()
            val engine = MockEngine { respond("", HttpStatusCode.fromValue(status)) }

            val thrown = assertFailsWith<RelayTransportException> {
                pairWith(peer, engine, PairIdentity(userId = "alice"))
            }
            assertEquals(failure, thrown.failure, "status $status")
        }
    }

    @Test
    fun commandsRoundTripThroughThePairedTransport() = runTest {
        val peer = DesktopPeer.create()
        var seen: RemoteCommand? = null

        val engine = handshakeEngine(peer) { body ->
            val decoded = peer.decryptRaw(body)
            if ("challenge_echo" in decoded.keys) {
                peer.encrypt(InitialSyncResponse.serializer(), InitialSyncResponse(resp = "ok"))
            } else {
                seen = peer.decrypt(RemoteCommand.serializer(), body)
                peer.encrypt(
                    SessionListResponse.serializer(),
                    SessionListResponse(resp = "ok", hasMore = true),
                )
            }
        }

        val paired = pairWith(peer, engine, PairIdentity(userId = "alice"))
        val response: SessionListResponse = paired.transport.send(
            RemoteCommand(cmd = "list_sessions", requestId = "req-000000000042", limit = 20),
        )

        assertEquals("list_sessions", seen?.cmd)
        assertEquals(20, seen?.limit)
        assertTrue(response.hasMore)
        assertNull(response.message)
    }

    @Test
    fun aCommandErrorReplyBecomesRemoteRejected() = runTest {
        val peer = DesktopPeer.create()
        val engine = handshakeEngine(peer) { body ->
            if ("challenge_echo" in peer.decryptRaw(body).keys) {
                peer.encrypt(InitialSyncResponse.serializer(), InitialSyncResponse(resp = "ok"))
            } else {
                peer.encrypt(
                    SessionListResponse.serializer(),
                    SessionListResponse(resp = "error", message = "Workspace is closed."),
                )
            }
        }

        val paired = pairWith(peer, engine, PairIdentity(userId = "alice"))
        val failure = assertFailsWith<RelayTransportException> {
            paired.transport.send<SessionListResponse>(RemoteCommand(cmd = "list_sessions"))
        }

        assertEquals(RelayFailure.RemoteRejected("Workspace is closed."), failure.failure)
    }

    @Test
    fun commandLogsKeepRequestIdsShortAndRoomIdsTruncated() = runTest {
        val peer = DesktopPeer.create()
        val log = RecordingLog()
        val engine = handshakeEngine(peer) { body ->
            if ("challenge_echo" in peer.decryptRaw(body).keys) {
                peer.encrypt(InitialSyncResponse.serializer(), InitialSyncResponse(resp = "ok"))
            } else {
                peer.encrypt(SessionListResponse.serializer(), SessionListResponse(resp = "ok"))
            }
        }

        val paired = pairWith(peer, engine, PairIdentity(userId = "alice"), log = log)
        log.lines.clear()
        paired.transport.send<SessionListResponse>(
            RemoteCommand(cmd = "list_sessions", requestId = "session-abcdef-000000000042"),
        )

        val joined = log.lines.joinToString("\n")
        assertContains(joined, "request=000000000042")
        assertFalse("session-abcdef" in joined)
        assertFalse(ROOM_ID in joined)
    }

    // --- helpers -----------------------------------------------------------

    private fun descriptorFor(publicKey: String) = RelayDescriptor(
        relayUrl = RELAY_URL,
        roomId = ROOM_ID,
        publicKey = publicKey,
        accountAuth = false,
        accountUsername = "",
    )

    private suspend fun pairWith(
        peer: DesktopPeer,
        engine: MockEngine,
        identity: PairIdentity,
        log: TransportLog = TransportLog.None,
    ): PairedRoom = RelayPairing(relayHttpClient(engine), log).pair(
        descriptor = descriptorFor(peer.publicKeyBase64),
        deviceId = DEVICE_ID,
        deviceName = DEVICE_NAME,
        identity = identity,
    )

    /**
     * A relay that always answers the challenge and delegates `/command` to
     * [onCommand], which receives the raw encrypted body.
     */
    private fun handshakeEngine(
        peer: DesktopPeer,
        onCommand: suspend (String) -> String,
    ) = MockEngine { request ->
        val body = request.text()
        when (request.path()) {
            "/api/rooms/$ROOM_ID/pair" -> {
                peer.acceptPairRequest(body)
                jsonResponse(
                    peer.encrypt(
                        PairChallengeResponse.serializer(),
                        PairChallengeResponse(DesktopPeer.CHALLENGE),
                    ),
                )
            }

            "/api/rooms/$ROOM_ID/command" -> jsonResponse(onCommand(body))
            else -> respond("", HttpStatusCode.NotFound)
        }
    }
}

private fun HttpRequestData.path(): String = url.encodedPath

private suspend fun HttpRequestData.text(): String = body.toByteArray().decodeToString()

private fun io.ktor.client.engine.mock.MockRequestHandleScope.jsonResponse(body: String) =
    respond(body, HttpStatusCode.OK, headersOf(HttpHeaders.ContentType, "application/json"))
