package com.openbitfun.mobile.core.transport

import com.openbitfun.mobile.core.crypto.CloudAccountCipher
import com.openbitfun.mobile.core.crypto.DeviceIdentity
import com.openbitfun.mobile.core.protocol.CommandStatusResponse
import com.openbitfun.mobile.core.protocol.EncryptedPayload
import com.openbitfun.mobile.core.protocol.RelayJson
import com.openbitfun.mobile.core.protocol.RemoteCommand
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.MockRequestHandleScope
import io.ktor.client.engine.mock.respond
import io.ktor.client.engine.mock.toByteArray
import io.ktor.client.request.HttpRequestData
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.io.encoding.Base64
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class CloudAccountClientTest {
    @Test
    fun authorizationAcceptsTheIdentityAuthorityGithubUrlAndRejectsOtherDestinations() = runTest {
        for (url in listOf("https://github.com/login/oauth/authorize?state=test", "https://github.com.evil.example/login/oauth/authorize", "https://github.com/login", "https://user@github.com/login/oauth/authorize", "http://github.com/login/oauth/authorize")) {
            val engine = MockEngine { json("""{"transactionId":"txn","transactionSecret":"secret","authorizationUrl":"$url","expiresAt":9999999999,"pollIntervalSeconds":3}""") }
            val client = CloudAccountClient(relayHttpClient(engine))
            if (url == "https://github.com/login/oauth/authorize?state=test") assertEquals(url, client.startAuthorization(DEFAULT_CLOUD_RELAY_URL).authorizationUrl)
            else assertFailsWith<IllegalArgumentException> { client.startAuthorization(DEFAULT_CLOUD_RELAY_URL) }
        }
    }

    @Test
    fun githubLoginRegistersOnlyThePublicDeviceKey() = runTest {
        val bodies = mutableListOf<kotlinx.serialization.json.JsonObject>()
        val engine = MockEngine { request ->
            assertEquals("https://remote.openbitfun.com/v/1.0.0/api/auth/login", request.url.toString())
            bodies += RelayJson.parseToJsonElement(request.text()).jsonObject
            json("""{"token":"token-1","user_id":"123"}""")
        }
        val client = CloudAccountClient(relayHttpClient(engine))
        val first = client.login(DEFAULT_CLOUD_RELAY_URL, "verified-identity", "device-1", "Android", ByteArray(32) { 7 })
        val second = client.login(DEFAULT_CLOUD_RELAY_URL, "verified-identity", "device-2", "iOS", ByteArray(32) { 11 })
        assertEquals("verified-identity", bodies[0]["access_token"]?.jsonPrimitive?.content)
        assertEquals(Base64.Default.encode(DeviceIdentity.publicKey(first.masterKey)), bodies[0]["public_key"]?.jsonPrimitive?.content)
        assertFalse(first.masterKey.contentEquals(second.masterKey))
        assertFalse(bodies[0].containsKey("password"))
        assertFalse(bodies[0].containsKey("master_key"))
        assertFalse(first.toString().contains("token-1"))
    }

    /**
     * Only desktops can be driven, so only desktops are offered. A row without a
     * kind comes from a relay that predates them: this device's own row and the
     * names our own builds register under are dropped anyway, and anything else
     * is kept rather than risk hiding a real desktop.
     */
    @Test
    fun listDevicesOffersDesktopsAndDropsPhones() = runTest {
        val engine = MockEngine {
            json(
                """[
                  {"device_id":"desktop-1","device_name":"Studio Mac","online":true,"device_kind":"desktop"},
                  {"device_id":"phone-2","device_name":"Pixel 8","online":true,"device_kind":"mobile"},
                  {"device_id":"watch-1","device_name":"Watch","online":false,"device_kind":"watch"},
                  {"device_id":"phone-1","device_name":"Pixel 8","online":true},
                  {"device_id":"harmony-phone","device_name":"HarmonyOS Phone","online":true},
                  {"device_id":"harmony-watch","device_name":"HarmonyOS Watch","online":false},
                  {"device_id":"phone-3","device_name":"Legacy Phone","online":true},
                  {"device_id":"watch-2","device_name":"Legacy Watch","online":false},
                  {"device_id":"legacy-1","device_name":"DESKTOP-KM3L4UI","online":false,"last_seen_at":9}
                ]""",
            )
        }
        val client = CloudAccountClient(
            relayHttpClient(engine),
            legacyMobileDeviceNames = setOf("Legacy Phone", "Legacy Watch"),
        )

        val devices = client.listDevices(
            "http://192.168.1.2:9700",
            CloudAccountSession("token-1", "user-1", ByteArray(32)),
            "phone-1",
        )

        assertEquals(listOf("desktop-1", "legacy-1"), devices.map { it.deviceId })
        assertEquals("desktop", devices[0].deviceKind)
        assertEquals(null, devices[1].deviceKind)
    }

    @Test
    fun accountDeviceTransportEncryptsCommandAndDecryptsResponse() = runTest {
        val masterKey = ByteArray(32) { it.toByte() }
        val session = CloudAccountSession("token-1", "user-1", masterKey)
        val peerSecret = ByteArray(32) { 11 }
        val peerPublic = DeviceIdentity.publicKey(peerSecret)
        val messageKey = DeviceIdentity.messageKey(peerSecret, DeviceIdentity.publicKey(masterKey))
        val engine = MockEngine { request ->
            assertEquals("Bearer token-1", request.headers[HttpHeaders.Authorization])
            if (request.url.encodedPath.endsWith("/key")) return@MockEngine json("""{"public_key":"${Base64.Default.encode(peerPublic)}"}""")
            val envelope = RelayJson.decodeFromString(EncryptedPayload.serializer(), request.text())
            val commandText = CloudAccountCipher.decrypt(
                Base64.Default.decode(envelope.encryptedData),
                messageKey,
                Base64.Default.decode(envelope.nonce),
            ).decodeToString()
            assertEquals("ping", RelayJson.decodeFromString(RemoteCommand.serializer(), commandText).cmd)
            val nonce = ByteArray(12) { (it + 20).toByte() }
            val plain = RelayJson.encodeToString(CommandStatusResponse.serializer(), CommandStatusResponse("ok", null))
            val encrypted = CloudAccountCipher.encrypt(plain.encodeToByteArray(), messageKey, nonce)
            json(
                RelayJson.encodeToString(
                    EncryptedPayload.serializer(),
                    EncryptedPayload(Base64.Default.encode(encrypted), Base64.Default.encode(nonce)),
                ),
            )
        }
        val client = CloudAccountClient(relayHttpClient(engine))
        val transport = AccountDeviceCommandTransport(client, "http://192.168.1.2:9700", session, "desktop 1")

        val response = transport.send<CommandStatusResponse>(RemoteCommand(cmd = "ping"))

        assertEquals("ok", response.resp)
    }

    /**
     * A desktop that answers `{"resp":"error"}` answered — the HTTP exchange
     * succeeded, so nothing below this notices. The paired transport has always
     * turned that into a rejection, and a caller cannot be asked to remember
     * which of the two it is talking to.
     */
    @Test
    fun accountDeviceTransportReportsARefusalRatherThanReturningIt() = runTest {
        val masterKey = ByteArray(32) { it.toByte() }
        val session = CloudAccountSession("token-1", "user-1", masterKey)
        val peerSecret = ByteArray(32) { 11 }
        val peerPublic = DeviceIdentity.publicKey(peerSecret)
        val messageKey = DeviceIdentity.messageKey(peerSecret, DeviceIdentity.publicKey(masterKey))
        val engine = MockEngine { request ->
            if (request.url.encodedPath.endsWith("/key")) return@MockEngine json("""{"public_key":"${Base64.Default.encode(peerPublic)}"}""")
            val nonce = ByteArray(12) { (it + 20).toByte() }
            val plain = RelayJson.encodeToString(
                CommandStatusResponse.serializer(),
                CommandStatusResponse("error", "No workspace is open"),
            )
            val encrypted = CloudAccountCipher.encrypt(plain.encodeToByteArray(), messageKey, nonce)
            json(
                RelayJson.encodeToString(
                    EncryptedPayload.serializer(),
                    EncryptedPayload(Base64.Default.encode(encrypted), Base64.Default.encode(nonce)),
                ),
            )
        }
        val transport = AccountDeviceCommandTransport(
            CloudAccountClient(relayHttpClient(engine)),
            "http://192.168.1.2:9700",
            session,
            "desktop-1",
        )

        val error = assertFailsWith<RelayTransportException> {
            transport.send<CommandStatusResponse>(RemoteCommand(cmd = "list_sessions"))
        }

        assertEquals(RelayFailure.RemoteRejected("No workspace is open"), error.failure)
    }

    /**
     * The account path speaks [CloudAccountFailure] and everything above a
     * transport speaks [RelayFailure]; the translation belongs here, or a screen
     * shared with the paired path can only report "something went wrong".
     */
    @Test
    fun accountDeviceTransportTranslatesRelayStatusIntoATypedFailure() = runTest {
        val session = CloudAccountSession("token-1", "user-1", ByteArray(32))
        val engine = MockEngine { respond("upstream is down", HttpStatusCode.ServiceUnavailable) }
        val transport = AccountDeviceCommandTransport(
            CloudAccountClient(relayHttpClient(engine)),
            "http://192.168.1.2:9700",
            session,
            "desktop-1",
        )

        val error = assertFailsWith<RelayTransportException> {
            transport.send<CommandStatusResponse>(RemoteCommand(cmd = "list_sessions"))
        }

        assertEquals(RelayFailure.RelayUnavailable(500), error.failure)
        assertEquals(CloudAccountFailure.RELAY_UNAVAILABLE, (error.cause as CloudAccountException).failure)
    }

    /**
     * The reply is the user's own sessions, so the reason a decode failed has to
     * be assembled from the schema rather than quoted from the document.
     */
    @Test
    fun decodeDetailNamesTheFieldWithoutQuotingThePayload() {
        val secret = "a session title nobody else should read"
        val cause = assertFailsWith<Throwable> {
            RelayJson.decodeFromString(
                RemoteWorkspaceProbe.serializer(),
                """{"title":"$secret"}""",
            )
        }

        val detail = decodeDetail(cause)

        assertTrue(detail.contains("missing="), detail)
        assertTrue(detail.contains("path"), detail)
        assertFalse(detail.contains(secret), detail)
    }
}

/** A required field the fixture above deliberately omits. */
@kotlinx.serialization.Serializable
private data class RemoteWorkspaceProbe(val path: String, val title: String)

private suspend fun HttpRequestData.text(): String = body.toByteArray().decodeToString()

private fun MockRequestHandleScope.json(body: String) =
    respond(body, HttpStatusCode.OK, headersOf(HttpHeaders.ContentType, "application/json"))
