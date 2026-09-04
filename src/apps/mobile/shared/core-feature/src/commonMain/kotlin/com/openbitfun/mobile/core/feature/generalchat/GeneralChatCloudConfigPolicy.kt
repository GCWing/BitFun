package com.openbitfun.mobile.core.feature.generalchat

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.booleanOrNull

/**
 * One model this chat can actually talk to, as it came off the account.
 *
 * [apiKey] never leaves the core: it is read when a request is built and is
 * absent from every UI shape, which is why this type is internal and
 * [GeneralChatModelUi] is what a screen sees.
 */
internal data class GeneralChatRuntimeModel(
    val modelId: String,
    val name: String,
    val provider: String,
    val apiUrl: String,
    val modelName: String,
    val apiKey: String,
) {
    override fun toString(): String = "GeneralChatRuntimeModel(modelId=$modelId, apiKey=<redacted>)"
}

/**
 * The account's settings document, reduced to the models this chat can use.
 *
 * Ported from `services/general-chat/GeneralChatCloudConfigPolicy.ets`. The
 * document is the desktop's, and the desktop configures far more than a phone
 * can drive — so most of the work here is refusing things: a model the user
 * disabled, one whose credential is a subscription the phone cannot present,
 * one missing an endpoint or a key. What survives is ordered the way the source
 * orders it, because the first entry is what a phone with no local model of its
 * own will end up using.
 *
 * Read through [JsonObject] rather than through generated serializers on
 * purpose. The source parses this with `JSON.parse` and reads properties off
 * whatever comes back, so a field of an unexpected type costs it that one field;
 * a typed decode would instead throw and silently cost the user every model they
 * have. The blob is written by another product on its own release schedule, and
 * this side degrades one field at a time.
 */
internal object GeneralChatCloudConfigPolicy {
    fun models(payload: String): List<GeneralChatRuntimeModel> {
        val root = try {
            JSON.parseToJsonElement(payload) as? JsonObject
        } catch (_: Throwable) {
            null
        } ?: return emptyList()
        // `parsed.config?.ai || parsed.ai`: the desktop has moved this key once
        // already, and both spellings are still in the wild.
        val ai = root.child("config")?.child("ai") ?: root.child("ai") ?: return emptyList()
        val wires = ai.children("models")
        val primaryId = ai.child("default_models")?.text("primary").orEmpty()

        // Three passes, not a sort: "the account's own default, then anything
        // meant for general chat, then whatever is left" is an order of
        // preference rather than of rank, and each pass may add nothing.
        val ordered = mutableListOf<JsonObject>()
        if (primaryId.isNotEmpty()) {
            wires.forEach { model -> if (model.text("id") == primaryId) pushCompatible(ordered, model) }
        }
        wires.forEach { model -> if (model.text("category") == GENERAL_CHAT_CATEGORY) pushCompatible(ordered, model) }
        wires.forEach { model -> pushCompatible(ordered, model) }

        return ordered.map { model ->
            val provider = model.text("provider").trim()
            val modelName = model.text("model_name").trim()
            // A model with no id of its own is still usable; it just has to be
            // named after what it is, or two of them would collide on "cloud:".
            val sourceId = model.text("id").trim().ifEmpty { provider + ":" + modelName }
            GeneralChatRuntimeModel(
                modelId = CLOUD_MODEL_PREFIX + sourceId,
                name = model.text("name").trim().ifEmpty { modelName },
                provider = provider.ifEmpty { ACCOUNT_PROVIDER },
                apiUrl = normalizedApiUrl(model),
                modelName = modelName,
                apiKey = model.text("api_key").trim(),
            )
        }
    }

    private fun pushCompatible(target: MutableList<JsonObject>, model: JsonObject) {
        if (!isCompatible(model)) return
        val id = model.text("id")
        if (target.none { it.text("id") == id }) target.add(model)
    }

    /**
     * Whether this phone could actually send a request to it.
     *
     * `subscription` is refused rather than attempted: that credential is the
     * desktop's own session with the vendor, and the phone has nothing to
     * present in its place — a model listed here that answers 401 on first use
     * is worse than one that was never listed.
     */
    private fun isCompatible(model: JsonObject): Boolean {
        if (!model.flag("enabled") || model.child("auth")?.text("type") == SUBSCRIPTION_AUTH) return false
        return model.text("base_url").trim().isNotEmpty() &&
            model.text("model_name").trim().isNotEmpty() &&
            model.text("api_key").trim().isNotEmpty()
    }

    /**
     * The endpoint to post to, which is not always the one that was configured.
     *
     * The desktop stores Anthropic's host bare and appends the path itself; this
     * client posts to whatever it is given, so the path has to be here. OpenBitFun's
     * own relay is left alone — it speaks the same protocol on its own route.
     */
    private fun normalizedApiUrl(model: JsonObject): String {
        val baseUrl = model.text("base_url").trim().trimEnd('/')
        val provider = model.text("provider").trim().lowercase()
        val needsMessagesPath = provider == ANTHROPIC_PROVIDER &&
            !baseUrl.contains(OPENBITFUN_HOST) &&
            !baseUrl.endsWith(ANTHROPIC_MESSAGES_PATH)
        return if (needsMessagesPath) baseUrl + ANTHROPIC_MESSAGES_PATH else baseUrl
    }

    private fun JsonObject.child(key: String): JsonObject? = this[key] as? JsonObject

    private fun JsonObject.children(key: String): List<JsonObject> =
        (this[key] as? JsonArray)?.filterIsInstance<JsonObject>().orEmpty()

    /** Mirrors the source's `typeof value === 'string' ? value : ''`. */
    private fun JsonObject.text(key: String): String =
        (this[key] as? JsonPrimitive)?.takeIf { it.isString }?.content.orEmpty()

    /** Mirrors `model.enabled !== true`: anything but a literal true is a no. */
    private fun JsonObject.flag(key: String): Boolean =
        (this[key] as? JsonPrimitive)?.booleanOrNull == true

    private val JSON = Json { ignoreUnknownKeys = true }

    private const val GENERAL_CHAT_CATEGORY = "general_chat"
    private const val SUBSCRIPTION_AUTH = "subscription"
    private const val ACCOUNT_PROVIDER = "account"
    private const val ANTHROPIC_PROVIDER = "anthropic"
    private const val ANTHROPIC_MESSAGES_PATH = "/v1/messages"
    private const val OPENBITFUN_HOST = "openbitfun.com"

    /** Namespaces an account model's id so it can never collide with the local one. */
    const val CLOUD_MODEL_PREFIX: String = "cloud:"
}
