package com.openbitfun.mobile.core.transport

import com.openbitfun.mobile.core.crypto.CloudAccountCipher
import com.openbitfun.mobile.core.crypto.CloudAccountKdfParams
import com.openbitfun.mobile.core.crypto.PlatformArgon2id
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
    fun completesChallengeLoginWithoutSendingPlaintextPassword() = runTest {
        val password = "correct horse battery staple"
        val salt = ByteArray(16) { it.toByte() }
        val kdfSalt = ByteArray(16) { (it + 16).toByte() }
        val params = CloudAccountKdfParams(8 * 1024, 1, 1)
        val masterKey = ByteArray(32) { (it + 32).toByte() }
        val nonce = ByteArray(12) { (it + 64).toByte() }
        val kek = PlatformArgon2id.derive(password, salt, params)
        val wrapped = CloudAccountCipher.encrypt(masterKey, kek, nonce)
        val requests = mutableListOf<String>()
        val engine = MockEngine { request ->
            val body = request.text()
            requests += body
            when (request.url.encodedPath) {
                "/relay/api/auth/login/challenge" -> json(
                    """{"salt":"${Base64.Default.encode(salt)}","kdf_salt":"${Base64.Default.encode(kdfSalt)}","argon2_params":"{\"m\":8192,\"t\":1,\"p\":1}","wrapped_master_key":"${Base64.Default.encode(wrapped)}.${Base64.Default.encode(nonce)}"}""",
                )
                "/relay/api/auth/login" -> json("""{"token":"token-1","user_id":"user-1"}""")
                else -> respond("", HttpStatusCode.NotFound)
            }
        }
        val client = CloudAccountClient(relayHttpClient(engine))

        val session = client.login("https://relay.test/relay", " user-1 ", password, "device-1", "Android")

        assertEquals("user-1", session.userId)
        assertContentEquals(masterKey, session.masterKey)
        assertFalse(requests.any { it.contains(password) })
        val expectedProof = Base64.Default.encode(PlatformArgon2id.derive(password, kdfSalt, params))
        assertEquals(expectedProof, RelayJson.parseToJsonElement(requests[1]).jsonObject["password_hash"]?.jsonPrimitive?.content)
        // Registering as a phone is what keeps this device out of every other
        // device's list of things it can drive.
        assertEquals("mobile", RelayJson.parseToJsonElement(requests[1]).jsonObject["device_kind"]?.jsonPrimitive?.content)
        assertFalse(session.toString().contains("token-1"))
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
            "https://relay.test/relay",
            CloudAccountSession("token-1", "user-1", ByteArray(32)),
            "phone-1",
        )

        assertEquals(listOf("desktop-1", "legacy-1"), devices.map { it.deviceId })
        assertEquals("desktop", devices[0].deviceKind)
        assertEquals(null, devices[1].deviceKind)
    }

    @Test
    fun fetchSettingsDecryptsTheAccountsDocument() = runTest {
        val masterKey = ByteArray(32) { it.toByte() }
        val nonce = ByteArray(12) { (it + 90).toByte() }
        val document = """{"config":{"ai":{"models":[]}}}"""
        val engine = MockEngine { request ->
            assertEquals("/relay/api/sync/settings", request.url.encodedPath)
            assertEquals("Bearer token-1", request.headers[HttpHeaders.Authorization])
            val sealed = CloudAccountCipher.encrypt(document.encodeToByteArray(), masterKey, nonce)
            json(
                """{"encrypted_data":"${Base64.Default.encode(sealed)}","nonce":"${Base64.Default.encode(nonce)}","version":7}""",
            )
        }
        val client = CloudAccountClient(relayHttpClient(engine))

        val blob = client.fetchSettings("https://relay.test/relay", CloudAccountSession("token-1", "user-1", masterKey))

        assertEquals(document, blob?.plaintext)
        assertEquals(7, blob?.version)
        // The document carries the user's provider keys, so it must not be one
        // careless log line away from a bug report.
        assertFalse(blob.toString().contains("models"))
    }

    /**
     * An account that has never synced has no settings, which is not a failure:
     * the relay says 404 and the phone simply has no account models to list.
     * An empty entry is the same answer written differently.
     */
    @Test
    fun fetchSettingsTreatsAMissingDocumentAsNoDocument() = runTest {
        val session = CloudAccountSession("token-1", "user-1", ByteArray(32))
        val absent = CloudAccountClient(relayHttpClient(MockEngine { respond("", HttpStatusCode.NotFound) }))
        val blank = CloudAccountClient(
            relayHttpClient(MockEngine { json("""{"encrypted_data":"","nonce":"","version":1}""") }),
        )

        assertEquals(null, absent.fetchSettings("https://relay.test/relay", session))
        assertEquals(null, blank.fetchSettings("https://relay.test/relay", session))
    }

    @Test
    fun fetchSettingsReportsADocumentItCannotOpen() = runTest {
        val engine = MockEngine {
            json("""{"encrypted_data":"${Base64.Default.encode(ByteArray(48))}","nonce":"${Base64.Default.encode(ByteArray(12))}","version":2}""")
        }
        val client = CloudAccountClient(relayHttpClient(engine))

        val error = assertFailsWith<CloudAccountException> {
            client.fetchSettings("https://relay.test/relay", CloudAccountSession("token-1", "user-1", ByteArray(32)))
        }

        assertEquals(CloudAccountFailure.MALFORMED_RESPONSE, error.failure)
    }

    @Test
    fun accountDeviceTransportEncryptsCommandAndDecryptsResponse() = runTest {
        val masterKey = ByteArray(32) { it.toByte() }
        val session = CloudAccountSession("token-1", "user-1", masterKey)
        val engine = MockEngine { request ->
            assertEquals("Bearer token-1", request.headers[HttpHeaders.Authorization])
            val envelope = RelayJson.decodeFromString(EncryptedPayload.serializer(), request.text())
            val commandText = CloudAccountCipher.decrypt(
                Base64.Default.decode(envelope.encryptedData),
                masterKey,
                Base64.Default.decode(envelope.nonce),
            ).decodeToString()
            assertEquals("ping", RelayJson.decodeFromString(RemoteCommand.serializer(), commandText).cmd)
            val nonce = ByteArray(12) { (it + 20).toByte() }
            val plain = RelayJson.encodeToString(CommandStatusResponse.serializer(), CommandStatusResponse("ok", null))
            val encrypted = CloudAccountCipher.encrypt(plain.encodeToByteArray(), masterKey, nonce)
            json(
                RelayJson.encodeToString(
                    EncryptedPayload.serializer(),
                    EncryptedPayload(Base64.Default.encode(encrypted), Base64.Default.encode(nonce)),
                ),
            )
        }
        val client = CloudAccountClient(relayHttpClient(engine))
        val transport = AccountDeviceCommandTransport(client, "https://relay.test/relay", session, "desktop 1")

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
        val engine = MockEngine {
            val nonce = ByteArray(12) { (it + 20).toByte() }
            val plain = RelayJson.encodeToString(
                CommandStatusResponse.serializer(),
                CommandStatusResponse("error", "No workspace is open"),
            )
            val encrypted = CloudAccountCipher.encrypt(plain.encodeToByteArray(), masterKey, nonce)
            json(
                RelayJson.encodeToString(
                    EncryptedPayload.serializer(),
                    EncryptedPayload(Base64.Default.encode(encrypted), Base64.Default.encode(nonce)),
                ),
            )
        }
        val transport = AccountDeviceCommandTransport(
            CloudAccountClient(relayHttpClient(engine)),
            "https://relay.test/relay",
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
            "https://relay.test/relay",
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
