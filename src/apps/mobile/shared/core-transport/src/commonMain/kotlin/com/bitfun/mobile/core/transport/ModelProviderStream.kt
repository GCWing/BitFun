package com.bitfun.mobile.core.transport

import com.bitfun.mobile.core.protocol.RelayJson
import io.ktor.client.HttpClient
import io.ktor.client.request.accept
import io.ktor.client.request.header
import io.ktor.client.request.preparePost
import io.ktor.client.request.setBody
import io.ktor.client.statement.bodyAsChannel
import io.ktor.http.ContentType
import io.ktor.http.HttpHeaders
import io.ktor.http.contentType
import io.ktor.utils.io.readLine
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

public enum class ModelProviderProtocol {
    ANTHROPIC,
    OPEN_AI,
}

public data class ModelProviderMessage public constructor(
    public val role: String,
    public val content: String,
)

public data class ModelProviderStreamResult public constructor(
    public val statusCode: Int,
    public val responseId: String,
    public val text: String,
)

public sealed interface ModelProviderFailure {
    public data object Authentication : ModelProviderFailure
    public data object RateLimited : ModelProviderFailure
    public data object ServiceUnavailable : ModelProviderFailure
    public data object InvalidResponse : ModelProviderFailure
    public data class Http public constructor(public val statusCode: Int) : ModelProviderFailure
}

public class ModelProviderException public constructor(
    public val failure: ModelProviderFailure,
) : IllegalStateException("Model provider request failed: $failure")

public class ModelProviderSseParser public constructor(
    private val protocol: ModelProviderProtocol,
) {
    private val frames = SseFrameDecoder()
    private var responseId: String = ""

    public fun feed(chunk: String): List<String> = frames.feed(chunk).flatMap(::parsePayload)

    public fun finish(): List<String> = frames.finish().flatMap(::parsePayload)

    public fun responseId(): String = responseId

    private fun parsePayload(payload: String): List<String> = try {
        val json = RelayJson.parseToJsonElement(payload).jsonObject
        if (protocol == ModelProviderProtocol.OPEN_AI) parseOpenAi(json) else parseAnthropic(json)
    } catch (_: Throwable) {
        emptyList()
    }

    private fun parseOpenAi(json: JsonObject): List<String> {
        if (responseId.isEmpty()) responseId = json["id"]?.jsonPrimitive?.contentOrNull.orEmpty()
        return json["choices"]?.jsonArray.orEmpty().mapNotNull { choice ->
            choice.jsonObject["delta"]?.jsonObject?.get("content")?.jsonPrimitive?.contentOrNull
        }.filter(String::isNotEmpty)
    }

    private fun parseAnthropic(json: JsonObject): List<String> {
        if (responseId.isEmpty()) {
            responseId = json["message"]?.jsonObject?.get("id")?.jsonPrimitive?.contentOrNull.orEmpty()
        }
        if (json["type"]?.jsonPrimitive?.contentOrNull != "content_block_delta") return emptyList()
        val delta = json["delta"]?.jsonObject ?: return emptyList()
        if (delta["type"]?.jsonPrimitive?.contentOrNull != "text_delta") return emptyList()
        return listOfNotNull(delta["text"]?.jsonPrimitive?.contentOrNull).filter(String::isNotEmpty)
    }
}

public object ModelProviderRequest {
    public fun resolveProtocol(baseUrl: String): ModelProviderProtocol {
        val normalized = baseUrl.trim().trimEnd('/').lowercase()
        return if ("openbitfun.com" in normalized || normalized.endsWith("/v1/messages")) {
            ModelProviderProtocol.ANTHROPIC
        } else {
            ModelProviderProtocol.OPEN_AI
        }
    }

    public fun resolveUrl(baseUrl: String, protocol: ModelProviderProtocol): String {
        val normalized = baseUrl.trim().trimEnd('/')
        return when (protocol) {
            ModelProviderProtocol.ANTHROPIC ->
                if (normalized.endsWith("/v1/messages")) normalized else "$normalized/v1/messages"
            ModelProviderProtocol.OPEN_AI -> when {
                normalized.endsWith("/chat/completions") -> normalized
                normalized.endsWith("/v1") -> "$normalized/chat/completions"
                else -> "$normalized/v1/chat/completions"
            }
        }
    }

    public fun body(
        model: String,
        messages: List<ModelProviderMessage>,
        protocol: ModelProviderProtocol,
        maxTokens: Int,
        systemPrompt: String,
    ): String {
        val providerMessages = when (protocol) {
            ModelProviderProtocol.ANTHROPIC -> messages
            ModelProviderProtocol.OPEN_AI -> listOf(ModelProviderMessage("system", systemPrompt)) + messages
        }
        val body = buildString {
            append("{\"model\":")
            append(jsonString(model))
            append(",\"max_tokens\":")
            append(maxTokens)
            append(",\"stream\":true")
            if (protocol == ModelProviderProtocol.ANTHROPIC) {
                append(",\"system\":")
                append(jsonString(systemPrompt))
            }
            append(",\"messages\":[")
            providerMessages.forEachIndexed { index, message ->
                if (index > 0) append(',')
                append("{\"role\":")
                append(jsonString(message.role))
                append(",\"content\":")
                append(jsonString(message.content))
                append('}')
            }
            append("]}")
        }
        return body
    }

    private fun jsonString(value: String): String =
        RelayJson.encodeToString(JsonPrimitive.serializer(), JsonPrimitive(value))
}

public class ModelProviderStreamClient internal constructor(
    private val client: HttpClient,
) {
    public suspend fun stream(
        baseUrl: String,
        apiKey: String,
        model: String,
        messages: List<ModelProviderMessage>,
        systemPrompt: String,
        maxTokens: Int,
        onDelta: (String) -> Unit,
    ): ModelProviderStreamResult {
        val protocol = ModelProviderRequest.resolveProtocol(baseUrl)
        val parser = ModelProviderSseParser(protocol)
        var text = ""
        val statement = client.preparePost(ModelProviderRequest.resolveUrl(baseUrl, protocol)) {
            contentType(ContentType.Application.Json)
            accept(ContentType.Text.EventStream)
            when (protocol) {
                ModelProviderProtocol.ANTHROPIC -> {
                    header("x-api-key", apiKey)
                    header("anthropic-version", "2023-06-01")
                    if ("openbitfun.com" in baseUrl.lowercase()) header("X-Verification-Code", "from_bitfun")
                }
                ModelProviderProtocol.OPEN_AI -> header(HttpHeaders.Authorization, "Bearer $apiKey")
            }
            setBody(ModelProviderRequest.body(model, messages, protocol, maxTokens, systemPrompt))
        }
        var status = 0
        statement.execute { response ->
            status = response.status.value
            if (status !in 200..299) throw ModelProviderException(statusFailure(status))
            val channel = response.bodyAsChannel()
            while (!channel.isClosedForRead) {
                val line = channel.readLine() ?: break
                parser.feed("$line\n").forEach { delta ->
                    text += delta
                    onDelta(delta)
                }
            }
        }
        parser.finish().forEach { delta -> text += delta; onDelta(delta) }
        if (text.isBlank()) throw ModelProviderException(ModelProviderFailure.InvalidResponse)
        return ModelProviderStreamResult(status, parser.responseId(), text)
    }

    public companion object {
        public fun create(): ModelProviderStreamClient = ModelProviderStreamClient(relayHttpClient())
    }
}

private fun statusFailure(status: Int): ModelProviderFailure = when (status) {
    401, 403 -> ModelProviderFailure.Authentication
    429 -> ModelProviderFailure.RateLimited
    in 500..599 -> ModelProviderFailure.ServiceUnavailable
    else -> ModelProviderFailure.Http(status)
}
