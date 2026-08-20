package com.bitfun.mobile.core.transport

import com.bitfun.mobile.core.protocol.CreateSessionResponse
import com.bitfun.mobile.core.protocol.ModelCatalogResponse
import com.bitfun.mobile.core.protocol.PollSessionResponse
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.protocol.SendMessageResponse
import com.bitfun.mobile.core.protocol.SessionListResponse
import com.bitfun.mobile.core.protocol.SessionMessagesResponse
import com.bitfun.mobile.core.protocol.WorkspaceInfoResponse
import io.ktor.client.engine.java.Java
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * End-to-end handshake and command round trip against a live
 * `harmonyos/tools/fake-relay.mjs`, over the real HTTP engine.
 *
 * Opt-in, because it needs a running node process: start the stub, then pass its
 * printed pairing URL through
 * `./gradlew :core-transport:jvmTest -Pbitfun.pairingUrl='<url>'`.
 *
 * Everything this covers is also covered by [RelayPairingTest] against an
 * in-process peer, so CI stays green without node. What only this test can show
 * is that the two implementations agree — that the Kotlin client and the stub
 * derive the same key, and that the DTOs match a body neither of them wrote.
 */
class FakeRelayIntegrationTest {
    private val pairingUrl: String = System.getProperty("bitfun.pairingUrl").orEmpty()

    @Test
    fun pairsAndRoundTripsCommands() = runTest {
        if (pairingUrl.isEmpty()) {
            println("skipping: pass -Pbitfun.pairingUrl=<fake-relay pairing url> to run")
            return@runTest
        }

        val descriptor = RelayDescriptorParser.parse(pairingUrl)
        val client = relayHttpClient(Java.create())
        val paired = RelayPairing(client).pair(
            descriptor = descriptor,
            deviceId = "jvm-integration-test",
            deviceName = "JVM Integration Test",
            identity = PairIdentity(userId = "jvm-integration-test"),
        )

        assertEquals("ok", paired.initialSync.resp)
        assertTrue(paired.initialSync.hasWorkspace == true)

        val transport = paired.transport

        val workspace: WorkspaceInfoResponse =
            transport.send(RemoteCommand(cmd = "get_workspace_info"))
        assertEquals("ok", workspace.resp)

        val sessions: SessionListResponse =
            transport.send(RemoteCommand(cmd = "list_sessions", limit = 8))
        assertEquals("ok", sessions.resp)
        assertTrue(sessions.sessions.isNotEmpty())

        val catalog: ModelCatalogResponse =
            transport.send(RemoteCommand(cmd = "get_model_catalog"))
        assertNotNull(catalog.catalog)

        val created: CreateSessionResponse = transport.send(
            RemoteCommand(cmd = "create_session", agentType = "code", sessionName = "integration"),
        )
        val sessionId = assertNotNull(created.resolvedSessionId)

        val messages: SessionMessagesResponse = transport.send(
            RemoteCommand(cmd = "get_session_messages", sessionId = sessionId, limit = 50),
        )
        assertEquals("ok", messages.resp)

        val sent: SendMessageResponse = transport.send(
            RemoteCommand(cmd = "send_message", sessionId = sessionId, content = "hello"),
        )
        assertNotNull(sent.turnId)

        // The cursor contract: send back the version the peer last reported and
        // it answers with what changed after it, never re-sending the same turn.
        var cursor = 0
        repeat(3) {
            val poll: PollSessionResponse = transport.send(
                RemoteCommand(cmd = "poll_session", sessionId = sessionId, sinceVersion = cursor),
            )
            assertEquals("ok", poll.resp)
            assertTrue(poll.version >= cursor, "cursor went backwards: ${poll.version} < $cursor")
            cursor = poll.version
        }
        assertTrue(cursor > 0, "poll_session never advanced its cursor")

        client.close()
    }
}
