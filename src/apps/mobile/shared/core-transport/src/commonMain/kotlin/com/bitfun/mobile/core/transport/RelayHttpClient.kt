package com.bitfun.mobile.core.transport

import com.bitfun.mobile.core.protocol.EncryptedPayload
import com.bitfun.mobile.core.protocol.RelayJson
import io.ktor.client.HttpClient
import io.ktor.client.HttpClientConfig
import io.ktor.client.engine.HttpClientEngine
import io.ktor.client.network.sockets.ConnectTimeoutException
import io.ktor.client.network.sockets.SocketTimeoutException
import io.ktor.client.plugins.HttpRequestTimeoutException
import io.ktor.client.plugins.HttpTimeout
import io.ktor.client.plugins.timeout
import io.ktor.client.request.accept
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.client.statement.bodyAsText
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.coroutines.CancellationException
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json

/** Connect budget, carried over from the HarmonyOS client. */
public const val RELAY_CONNECT_TIMEOUT_MS: Long = 15_000

/** Default per-request budget; long-running commands raise it per call. */
public const val RELAY_DEFAULT_TIMEOUT_MS: Long = 30_000

/**
 * Builds an [HttpClient] configured for the relay.
 *
 * The module owns client construction rather than accepting an arbitrary one
 * because the timeouts are part of the ported behaviour: without the
 * [HttpTimeout] plugin installed, a per-request `timeout { }` block is silently
 * a no-op on some engines and throws on others.
 *
 * `expectSuccess` stays off so status codes reach [httpFailureFor] instead of
 * surfacing as ktor's own exception types.
 */
public fun relayHttpClient(engine: HttpClientEngine): HttpClient =
    HttpClient(engine) { configureForRelay() }

/** As [relayHttpClient], using the engine linked into the platform artifact. */
public fun relayHttpClient(): HttpClient = HttpClient { configureForRelay() }

private fun HttpClientConfig<*>.configureForRelay() {
    expectSuccess = false
    install(HttpTimeout) {
        connectTimeoutMillis = RELAY_CONNECT_TIMEOUT_MS
        requestTimeoutMillis = RELAY_DEFAULT_TIMEOUT_MS
    }
}

/**
 * The relay's two POST endpoints, with every outcome expressed as a
 * [RelayFailure].
 *
 * Bodies are serialised with [RelayJson] by hand rather than through
 * ContentNegotiation, so the wire format does not depend on which converters
 * the surrounding app happens to have installed.
 */
internal class RelayEndpoints(
    private val httpClient: HttpClient,
    private val relayUrl: String,
    private val json: Json = RelayJson,
) {
    suspend fun postForEncryptedPayload(
        path: String,
        body: String,
        timeoutMs: Long,
    ): EncryptedPayload {
        val text = post(path, body, timeoutMs)
        return try {
            json.decodeFromString(EncryptedPayload.serializer(), text)
        } catch (cause: SerializationException) {
            // The body is attacker-influenced and may contain anything; only the
            // fact that it did not parse is reportable.
            throw RelayTransportException(RelayFailure.MalformedResponse, cause)
        }
    }

    private suspend fun post(path: String, body: String, timeoutMs: Long): String {
        val response = try {
            httpClient.post("$relayUrl$path") {
                contentType(ContentType.Application.Json)
                accept(ContentType.Application.Json)
                setBody(body)
                timeout { requestTimeoutMillis = timeoutMs }
            }
        } catch (cancellation: CancellationException) {
            // Caller-side cancellation is not a transport failure and must keep
            // propagating as cancellation, or structured concurrency breaks.
            throw cancellation
        } catch (timeout: HttpRequestTimeoutException) {
            throw RelayTransportException(RelayFailure.Timeout, timeout)
        } catch (timeout: ConnectTimeoutException) {
            throw RelayTransportException(RelayFailure.Timeout, timeout)
        } catch (timeout: SocketTimeoutException) {
            throw RelayTransportException(RelayFailure.Timeout, timeout)
        } catch (cause: Throwable) {
            // DNS, TLS, refused connections and engine-specific I/O errors have
            // no common supertype across ktor's four engines.
            throw RelayTransportException(RelayFailure.NetworkUnreachable, cause)
        }

        val status = response.status.value
        if (status !in 200..299) throw RelayTransportException(httpFailureFor(status))
        return response.bodyAsText()
    }
}
