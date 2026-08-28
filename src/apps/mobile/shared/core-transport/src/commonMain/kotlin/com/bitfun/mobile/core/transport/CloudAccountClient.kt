package com.bitfun.mobile.core.transport

import com.bitfun.mobile.core.crypto.CloudAccountCipher
import com.bitfun.mobile.core.crypto.CloudAccountKdfParams
import com.bitfun.mobile.core.crypto.PlatformArgon2id
import com.bitfun.mobile.core.protocol.CommandStatus
import com.bitfun.mobile.core.protocol.EncryptedPayload
import com.bitfun.mobile.core.protocol.RelayJson
import com.bitfun.mobile.core.protocol.RemoteCommand
import com.bitfun.mobile.core.protocol.isError
import io.ktor.client.HttpClient
import io.ktor.client.plugins.HttpRequestTimeoutException
import io.ktor.client.request.accept
import io.ktor.client.request.bearerAuth
import io.ktor.client.request.request
import io.ktor.client.request.setBody
import io.ktor.client.statement.bodyAsText
import io.ktor.http.ContentType
import io.ktor.http.HttpMethod
import io.ktor.http.contentType
import io.ktor.client.plugins.timeout
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.MissingFieldException
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationStrategy
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonObject
import kotlinx.coroutines.CancellationException
import kotlin.io.encoding.Base64
import kotlin.random.Random

public const val DEFAULT_CLOUD_RELAY_URL: String = "https://remote.openbitfun.com/relay"

/** Device kinds the relay accepts; mirrors `relay-service/src/db.rs::DEVICE_KINDS`. */
private const val DEVICE_KIND_DESKTOP = "desktop"

/**
 * What this client registers itself as. Constant rather than a parameter: the
 * shared transport only ships inside the Android and iOS apps, and a desktop
 * never reaches the relay through it.
 */
private const val DEVICE_KIND_MOBILE = "mobile"

/**
 * Names used by our pre-`device_kind` mobile clients.
 *
 * This belongs to the shared transport rather than an Android/iOS adapter: both
 * native clients can receive the same legacy account rows, and both must hide
 * them from the desktop target picker.
 */
private val KNOWN_NON_DESKTOP_DEVICE_NAMES = setOf(
    "HarmonyOS Phone",
    "HarmonyOS Watch",
)

/**
 * Whether a relay device row is a desktop, and so controllable from a phone.
 *
 * A row that reports its kind is taken at its word. A row without one predates
 * the relay learning about kinds, and is judged by two weaker signals: this
 * phone's own row is never a desktop, and neither is one carrying a name our
 * own builds register under. Anything else stays visible — hiding a real
 * desktop would strand the user, while a stale phone row disappears the next
 * time that phone logs in against a relay that stores kinds.
 */
private fun AccountDeviceWire.isDesktop(
    selfDeviceId: String,
    isLegacyMobileDeviceName: (String) -> Boolean,
): Boolean {
    val kind = deviceKind?.trim().orEmpty()
    if (kind.isNotEmpty()) return kind == DEVICE_KIND_DESKTOP
    if (selfDeviceId.isNotEmpty() && deviceId == selfDeviceId) return false
    return !isLegacyMobileDeviceName(deviceName)
}

public enum class CloudAccountFailure {
    INVALID_CREDENTIALS,
    AUTHENTICATION,
    RATE_LIMITED,
    RELAY_UNAVAILABLE,
    NETWORK,

    /**
     * The request left, and nothing came back before [timeoutMs] ran out.
     *
     * Separate from [NETWORK] because the two ask for different things: a
     * network error means try again, a timeout on a device RPC usually means the
     * desktop is working on something big and the wait was too short.
     */
    TIMEOUT,
    MALFORMED_RESPONSE,
}

public class CloudAccountException public constructor(
    public val failure: CloudAccountFailure,
    public val statusCode: Int?,
    cause: Throwable?,
) : IllegalStateException("Cloud account request failed: $failure", cause) {
    public constructor(failure: CloudAccountFailure, statusCode: Int?) : this(failure, statusCode, null)

    public constructor(failure: CloudAccountFailure) : this(failure, null, null)
}

public data class CloudAccountSession public constructor(
    public val token: String,
    public val userId: String,
    public val masterKey: ByteArray,
) {
    override fun toString(): String = "CloudAccountSession(token=<redacted>, userId=$userId, masterKey=<redacted>)"
}

public data class CloudAccountDevice public constructor(
    public val deviceId: String,
    public val deviceName: String,
    public val online: Boolean,
    public val lastSeenAt: Long?,
    /** `desktop`, or null for a row the relay stored before kinds existed. */
    public val deviceKind: String? = null,
)

/**
 * The account's settings, decrypted, still as the desktop wrote them.
 *
 * Kept as text rather than parsed here: the blob is the desktop's whole
 * configuration document and this layer has no business knowing its schema —
 * only that the relay stored it sealed and that the master key opens it.
 *
 * [plaintext] is redacted from [toString] because that document carries the
 * user's provider API keys.
 */
public data class CloudSettingsBlob public constructor(
    public val plaintext: String,
    public val version: Long,
) {
    override fun toString(): String = "CloudSettingsBlob(plaintext=<redacted>, version=$version)"
}

public class CloudAccountClient internal constructor(
    private val client: HttpClient,
    private val log: TransportLog = TransportLog.None,
    legacyMobileDeviceNames: Set<String> = emptySet(),
) {
    private val normalizedLegacyMobileDeviceNames =
        (KNOWN_NON_DESKTOP_DEVICE_NAMES + legacyMobileDeviceNames).mapTo(mutableSetOf()) {
            it.trim().lowercase()
        }

    public suspend fun login(
        relayUrl: String,
        username: String,
        password: String,
        deviceId: String,
        deviceName: String,
    ): CloudAccountSession {
        val relay = relayUrl.trim().ifEmpty { DEFAULT_CLOUD_RELAY_URL }.trimEnd('/')
        val user = username.trim()
        if (user.isEmpty() || user.length > 128 || password.isEmpty() || password.length > 1024) {
            throw CloudAccountException(CloudAccountFailure.INVALID_CREDENTIALS)
        }
        val challenge = request(
            relay,
            "/api/auth/login/challenge",
            HttpMethod.Post,
            LoginChallengeRequest.serializer(),
            LoginChallengeRequest(user),
            AccountChallenge.serializer(),
            "",
            RELAY_DEFAULT_TIMEOUT_MS,
        )
        val paramsWire = try {
            RelayJson.decodeFromString(Argon2ParamsWire.serializer(), challenge.argon2Params)
        } catch (_: Throwable) {
            throw CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE)
        }
        val params = CloudAccountKdfParams(paramsWire.m, paramsWire.t, paramsWire.p)
        val salt = decode(challenge.salt)
        val kdfSalt = decode(challenge.kdfSalt)
        val kek = PlatformArgon2id.derive(password, salt, params)
        val masterKey = unwrapMasterKey(kek, challenge.wrappedMasterKey)
        val proof = PlatformArgon2id.derive(password, kdfSalt, params)
        val auth = request(
            relay,
            "/api/auth/login",
            HttpMethod.Post,
            LoginRequest.serializer(),
            LoginRequest(user, Base64.Default.encode(proof), deviceId, deviceName, DEVICE_KIND_MOBILE),
            AccountAuthResponse.serializer(),
            "",
            RELAY_DEFAULT_TIMEOUT_MS,
        )
        if (auth.token.isEmpty() || auth.userId.isEmpty()) {
            throw CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE)
        }
        return CloudAccountSession(auth.token, auth.userId, masterKey)
    }

    /**
     * The account's controllable devices — desktops only.
     *
     * A relay that stores device kinds already filters this list; the client
     * repeats the judgement so a phone stops listing itself and its peers
     * before that relay is deployed. [selfDeviceId] is this install's own id.
     */
    public suspend fun listDevices(
        relayUrl: String,
        session: CloudAccountSession,
        selfDeviceId: String = "",
    ): List<CloudAccountDevice> =
        requestWithoutBody(
            relayUrl.trim().ifEmpty { DEFAULT_CLOUD_RELAY_URL }.trimEnd('/'),
            "/api/devices",
            HttpMethod.Get,
            ListSerializer(AccountDeviceWire.serializer()),
            session.token,
            RELAY_DEFAULT_TIMEOUT_MS,
        ).filter { device ->
            device.isDesktop(selfDeviceId) { name ->
                name.trim().lowercase() in normalizedLegacyMobileDeviceNames
            }
        }.map { device ->
            CloudAccountDevice(
                device.deviceId,
                device.deviceName.ifEmpty { device.deviceId },
                device.online,
                device.lastSeenAt,
                device.deviceKind,
            )
        }

    /**
     * The account's encrypted settings, or null when the account has none.
     *
     * "None" has two spellings on this endpoint and both mean the same thing to
     * a caller: a relay that has never been given a settings document answers
     * 404, and one that holds an empty row answers 200 with nothing sealed in
     * it. `CloudAccountClient.ets` folds both into `undefined` rather than an
     * error, because an account that has simply not synced yet is an ordinary
     * state and not a failure anyone can act on.
     *
     * Anything else — an expired token, a relay that is down, a body that will
     * not decrypt — still throws, so a caller can tell "you have no models" from
     * "I could not find out".
     */
    public suspend fun fetchSettings(relayUrl: String, session: CloudAccountSession): CloudSettingsBlob? {
        val entry = try {
            requestWithoutBody(
                relayUrl.trim().ifEmpty { DEFAULT_CLOUD_RELAY_URL }.trimEnd('/'),
                "/api/sync/settings",
                HttpMethod.Get,
                SyncSettingsWire.serializer(),
                session.token,
                RELAY_DEFAULT_TIMEOUT_MS,
            )
        } catch (error: CloudAccountException) {
            if (error.statusCode == HTTP_NOT_FOUND) return null
            throw error
        }
        if (entry.encryptedData.isEmpty() || entry.nonce.isEmpty()) return null
        val plaintext = try {
            CloudAccountCipher.decrypt(
                decode(entry.encryptedData),
                session.masterKey,
                decode(entry.nonce),
            ).decodeToString()
        } catch (error: CloudAccountException) {
            throw error
        } catch (cause: Throwable) {
            // Neither the body nor the reason it would not open: this blob is
            // the user's provider credentials.
            log.error("account settings undecryptable version=${entry.version}")
            throw CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE, null, cause)
        }
        return CloudSettingsBlob(plaintext, entry.version)
    }

    public suspend fun <T : CommandStatus> deviceRpc(
        relayUrl: String,
        session: CloudAccountSession,
        targetDeviceId: String,
        command: RemoteCommand,
        deserializer: DeserializationStrategy<T>,
        timeoutMs: Long,
    ): T {
        val target = targetDeviceId.trim()
        if (target.isEmpty()) throw CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE)
        val nonce = Random.Default.nextBytes(12)
        val plain = RelayJson.encodeToString(RemoteCommand.serializer(), command).encodeToByteArray()
        val encrypted = CloudAccountCipher.encrypt(plain, session.masterKey, nonce)
        val response = request(
            relayUrl.trim().ifEmpty { DEFAULT_CLOUD_RELAY_URL }.trimEnd('/'),
            "/api/devices/" + encodePathSegment(target) + "/rpc",
            HttpMethod.Post,
            EncryptedPayload.serializer(),
            EncryptedPayload(Base64.Default.encode(encrypted), Base64.Default.encode(nonce)),
            EncryptedPayload.serializer(),
            session.token,
            timeoutMs,
        )
        val decoded = try {
            CloudAccountCipher.decrypt(
                decode(response.encryptedData),
                session.masterKey,
                decode(response.nonce),
            ).decodeToString()
        } catch (error: CloudAccountException) {
            throw error
        } catch (cause: Throwable) {
            log.error("device rpc undecryptable cmd=${command.cmd} reason=${cause::class.simpleName}")
            throw CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE, null, cause)
        }
        return try {
            RelayJson.decodeFromString(deserializer, decoded)
        } catch (cause: Throwable) {
            log.error("device rpc undecodable cmd=${command.cmd} bytes=${decoded.length} ${decodeDetail(cause)}")
            throw CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE, null, cause)
        }
    }

    private suspend fun unwrapMasterKey(kek: ByteArray, wrapped: String): ByteArray {
        val parts = wrapped.split('.')
        if (parts.size != 2 || kek.size != 32) {
            throw CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE)
        }
        return try {
            CloudAccountCipher.decrypt(decode(parts[0]), kek, decode(parts[1])).also { masterKey ->
                if (masterKey.size != 32) throw CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE)
            }
        } catch (error: CloudAccountException) {
            throw error
        } catch (_: Throwable) {
            throw CloudAccountException(CloudAccountFailure.AUTHENTICATION)
        }
    }

    private suspend fun <Request, Response> request(
        relayUrl: String,
        path: String,
        method: HttpMethod,
        serializer: SerializationStrategy<Request>,
        body: Request,
        deserializer: DeserializationStrategy<Response>,
        token: String,
        timeoutMs: Long,
    ): Response = execute(relayUrl, path, method, RelayJson.encodeToString(serializer, body), deserializer, token, timeoutMs)

    private suspend fun <Response> requestWithoutBody(
        relayUrl: String,
        path: String,
        method: HttpMethod,
        deserializer: DeserializationStrategy<Response>,
        token: String,
        timeoutMs: Long,
    ): Response = execute(relayUrl, path, method, null, deserializer, token, timeoutMs)

    private suspend fun <Response> execute(
        relayUrl: String,
        path: String,
        method: HttpMethod,
        body: String?,
        deserializer: DeserializationStrategy<Response>,
        token: String,
        timeoutMs: Long,
    ): Response {
        val response = try {
            client.request(relayUrl + path) {
                this.method = method
                contentType(ContentType.Application.Json)
                accept(ContentType.Application.Json)
                if (token.isNotEmpty()) bearerAuth(token)
                if (body != null) setBody(body)
                timeout { requestTimeoutMillis = timeoutMs }
            }
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (timedOut: HttpRequestTimeoutException) {
            log.warn("account request timed out path=$path after=${timeoutMs}ms")
            throw CloudAccountException(CloudAccountFailure.TIMEOUT, null, timedOut)
        } catch (cause: Throwable) {
            log.error("account request failed path=$path reason=${cause::class.simpleName}")
            throw CloudAccountException(CloudAccountFailure.NETWORK, null, cause)
        }
        val text = response.bodyAsText()
        if (response.status.value !in 200..299) {
            log.warn("account request rejected path=$path status=${response.status.value}")
            throw statusFailure(response.status.value)
        }
        return try {
            RelayJson.decodeFromString(deserializer, text)
        } catch (cause: Throwable) {
            // The body itself is never logged: it carries whatever the desktop
            // was asked for, and on this path that is the user's own sessions.
            log.error("account response undecodable path=$path bytes=${text.length} ${decodeDetail(cause)}")
            throw CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE, null, cause)
        }
    }

    public companion object {
        public fun create(): CloudAccountClient = CloudAccountClient(relayHttpClient(), TransportLog.None)

        public fun create(log: TransportLog): CloudAccountClient = CloudAccountClient(relayHttpClient(), log)

        public fun create(
            log: TransportLog,
            legacyMobileDeviceNames: Set<String>,
        ): CloudAccountClient = CloudAccountClient(relayHttpClient(), log, legacyMobileDeviceNames)
    }
}

/**
 * Command transport over a signed-in account, i.e. `POST /api/devices/{id}/rpc`.
 *
 * The envelope differs from [RoomRemoteCommandTransport]'s — the key is the
 * account's master key rather than a pairing handshake's — but everything above
 * a transport is written against one contract, so the two failure vocabularies
 * are reconciled here rather than left for each caller to learn: a
 * [CloudAccountException] becomes the [RelayFailure] that says the same thing,
 * and a desktop that answered `{"resp":"error"}` is a rejection rather than a
 * reply, exactly as it is on the paired path.
 */
public class AccountDeviceCommandTransport public constructor(
    private val client: CloudAccountClient,
    private val relayUrl: String,
    private val session: CloudAccountSession,
    private val targetDeviceId: String,
    private val log: TransportLog,
) : RemoteCommandTransport {
    public constructor(
        client: CloudAccountClient,
        relayUrl: String,
        session: CloudAccountSession,
        targetDeviceId: String,
    ) : this(client, relayUrl, session, targetDeviceId, TransportLog.None)

    override suspend fun <T : CommandStatus> send(
        deserializer: DeserializationStrategy<T>,
        command: RemoteCommand,
        timeoutMs: Long,
    ): T {
        val label = "cmd=${command.cmd} request=${shortRequestId(command.requestId.orEmpty())} " +
            "device=${shortRoomId(targetDeviceId)}"
        log.info("command start $label")

        val response = try {
            client.deviceRpc(relayUrl, session, targetDeviceId, command, deserializer, timeoutMs)
        } catch (error: CloudAccountException) {
            log.warn("command failed $label failure=${error.failure}")
            throw RelayTransportException(error.failure.asRelayFailure(), error)
        }

        if (response.isError) {
            // The desktop's own sentence, already localized there. Echoed to the
            // user, never matched on — same rule as the paired transport.
            log.warn("command rejected $label")
            throw RelayTransportException(RelayFailure.RemoteRejected(response.message))
        }
        log.info("command done $label resp=${response.resp ?: "unknown"}")
        return response
    }
}

private fun CloudAccountFailure.asRelayFailure(): RelayFailure = when (this) {
    CloudAccountFailure.INVALID_CREDENTIALS, CloudAccountFailure.AUTHENTICATION -> RelayFailure.PairRejected
    CloudAccountFailure.RATE_LIMITED -> RelayFailure.RateLimited
    CloudAccountFailure.RELAY_UNAVAILABLE -> RelayFailure.RelayUnavailable(HTTP_SERVER_ERROR)
    CloudAccountFailure.NETWORK -> RelayFailure.NetworkUnreachable
    CloudAccountFailure.TIMEOUT -> RelayFailure.Timeout
    CloudAccountFailure.MALFORMED_RESPONSE -> RelayFailure.MalformedResponse
}

/**
 * [CloudAccountFailure] keeps the status on the exception rather than in the
 * value, and [RelayFailure.RelayUnavailable] wants one — this stands in for the
 * whole 5xx band, which is all that value is ever switched on.
 */
private const val HTTP_SERVER_ERROR = 500

@Serializable
private data class LoginChallengeRequest(val username: String)

@Serializable
private data class LoginRequest(
    val username: String,
    @SerialName("password_hash") val passwordHash: String,
    @SerialName("device_id") val deviceId: String,
    @SerialName("device_name") val deviceName: String,
    @SerialName("device_kind") val deviceKind: String,
)

@Serializable
private data class AccountChallenge(
    val salt: String,
    @SerialName("kdf_salt") val kdfSalt: String,
    @SerialName("argon2_params") val argon2Params: String,
    @SerialName("wrapped_master_key") val wrappedMasterKey: String,
)

@Serializable
private data class Argon2ParamsWire(val m: Int, val t: Int, val p: Int)

@Serializable
private data class AccountAuthResponse(val token: String, @SerialName("user_id") val userId: String)

/**
 * Defaults on every field: the relay answers this endpoint with an empty object
 * for an account that has never synced, and that has to read as "nothing here"
 * rather than as a malformed response.
 */
@Serializable
private data class SyncSettingsWire(
    @SerialName("encrypted_data") val encryptedData: String = "",
    val nonce: String = "",
    val version: Long = 0,
)

/** The relay's "this account has no settings document", not an error. */
private const val HTTP_NOT_FOUND = 404

@Serializable
private data class AccountDeviceWire(
    @SerialName("device_id") val deviceId: String,
    @SerialName("device_name") val deviceName: String,
    val online: Boolean,
    @SerialName("last_seen_at") val lastSeenAt: Long? = null,
    @SerialName("device_kind") val deviceKind: String? = null,
)

private fun decode(value: String): ByteArray = try {
    Base64.Default.withPadding(Base64.PaddingOption.PRESENT_OPTIONAL).decode(value)
} catch (_: Throwable) {
    throw CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE)
}

/**
 * What to say about a decode failure without quoting the payload.
 *
 * kotlinx puts the useful part — which field, at which position in the document —
 * into a message that also carries a slice of the input, and on this path that
 * input is the user's own sessions. So the message is never printed: the missing
 * field names come from [MissingFieldException]'s own list, and everything else
 * contributes only the `$.a.b[0]` path that kotlinx appends, which names the
 * schema rather than the data.
 */
@OptIn(ExperimentalSerializationApi::class)
internal fun decodeDetail(cause: Throwable): String {
    val kind = "reason=" + (cause::class.simpleName ?: "unknown")
    if (cause is MissingFieldException) {
        return kind + " missing=" + cause.missingFields.joinToString(",")
    }
    val path = cause.message?.let { JSON_PATH.find(it) }?.value
    return if (path == null) kind else "$kind at=$path"
}

private val JSON_PATH = Regex("""\$(\.[A-Za-z_][A-Za-z0-9_]*|\[\d+])+""")

private fun statusFailure(status: Int): CloudAccountException = when (status) {
    401, 403 -> CloudAccountException(CloudAccountFailure.AUTHENTICATION, status)
    429 -> CloudAccountException(CloudAccountFailure.RATE_LIMITED, status)
    in 500..599 -> CloudAccountException(CloudAccountFailure.RELAY_UNAVAILABLE, status)
    else -> CloudAccountException(CloudAccountFailure.MALFORMED_RESPONSE, status)
}

private fun encodePathSegment(value: String): String = value.encodeToByteArray().joinToString("") { byte ->
    val unsigned = byte.toInt() and 0xff
    val character = unsigned.toChar()
    if (character.isLetterOrDigit() || character in "-._~") character.toString()
    else "%" + unsigned.toString(16).uppercase().padStart(2, '0')
}
