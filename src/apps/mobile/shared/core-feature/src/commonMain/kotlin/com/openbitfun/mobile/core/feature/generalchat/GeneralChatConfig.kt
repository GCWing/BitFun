package com.openbitfun.mobile.core.feature.generalchat

import com.openbitfun.mobile.core.persistence.SecureStore

/**
 * The provider configuration as a screen needs to see it.
 *
 * [hasApiKey] rather than the key itself: a saved key is never handed back to
 * the UI layer, so it cannot end up in a saved instance state, a screenshot, or
 * a log line. The panel only ever needs to know whether one is there.
 */
public data class GeneralChatConfigUi public constructor(
    public val baseUrl: String,
    public val model: String,
    public val hasApiKey: Boolean,
)

/** Where a model in the catalog came from. */
public enum class GeneralChatModelSource {
    /** Typed into this phone's own provider form. */
    LOCAL,

    /** Synced down from the signed-in account's encrypted settings. */
    ACCOUNT,
}

/**
 * One model the chat could answer with, as a screen needs to see it.
 *
 * No endpoint and no credential: an account model arrives complete with an API
 * key, and the only reason a screen has to name one is to say which is in use.
 */
public data class GeneralChatModelUi public constructor(
    public val id: String,
    public val label: String,
    public val source: GeneralChatModelSource,
)

/** Everything one request needs, assembled at dispatch and dropped after. */
internal data class GeneralChatEndpoint(
    val baseUrl: String,
    val model: String,
    val apiKey: String,
) {
    override fun toString(): String = "GeneralChatEndpoint(model=$model, apiKey=<redacted>)"
}

/** Why a configuration was refused. The wording lives in each app's resources. */
public enum class GeneralChatConfigFailure {
    INVALID_URL,
    MODEL_REQUIRED,
    API_KEY_REQUIRED,
    SECURE_STORAGE,
}

/** Ported from `GeneralChatConfigValidator`; both clients refuse the same input. */
public object GeneralChatConfigValidator {
    public fun validate(
        baseUrl: String,
        model: String,
        apiKey: String,
        clearApiKey: Boolean,
        hasExistingApiKey: Boolean,
    ): GeneralChatConfigFailure? {
        if (!isHttpUrl(baseUrl.trim())) return GeneralChatConfigFailure.INVALID_URL
        if (model.trim().isEmpty()) return GeneralChatConfigFailure.MODEL_REQUIRED
        // A blank key keeps the stored one; only "clear it" or "there is none"
        // leaves the provider unreachable.
        if (apiKey.trim().isEmpty() && (clearApiKey || !hasExistingApiKey)) {
            return GeneralChatConfigFailure.API_KEY_REQUIRED
        }
        return null
    }

    private fun isHttpUrl(value: String): Boolean {
        val lower = value.lowercase()
        val scheme = when {
            lower.startsWith("https://") -> 8
            lower.startsWith("http://") -> 7
            else -> return false
        }
        val rest = value.substring(scheme)
        return rest.isNotEmpty() && !rest.contains(' ') && !rest.contains('#')
    }
}

/**
 * Where the provider settings live between launches.
 *
 * All three fields go through [SecureStore] even though only the key is a
 * secret. The base URL and model name are not worth a second storage backend,
 * and keeping them together means there is exactly one place that can hold
 * anything about the provider — no plain preference file to audit, and nothing
 * for a future edit to accidentally widen.
 */
internal class GeneralChatConfigStore(private val secure: SecureStore) {
    /**
     * The account's models, held only in memory.
     *
     * Each one carries the user's API key for a third-party provider, and this
     * copy is a cache of something the relay already stores sealed — writing it
     * anywhere on the device would add a second place to lose it from and buy
     * nothing but a faster cold start.
     */
    private var accountModels: List<GeneralChatRuntimeModel> = emptyList()

    fun snapshot(): GeneralChatConfigUi = GeneralChatConfigUi(
        baseUrl = read(BASE_URL_KEY),
        model = read(MODEL_KEY),
        hasApiKey = read(API_KEY_KEY).isNotEmpty(),
    )

    fun apiKey(): String = read(API_KEY_KEY)

    /** `replaceAccountModels`: the account is the whole truth about its own models. */
    fun replaceAccountModels(models: List<GeneralChatRuntimeModel>) {
        accountModels = models
    }

    /**
     * Every model that could answer right now, local first — `modelCatalog()`.
     *
     * The local model appears only when it is complete, because an endpoint with
     * no key is a form someone started rather than a model, and offering it
     * would make "which model is answering" answerable with one that cannot.
     */
    fun catalog(): List<GeneralChatModelUi> {
        val entries = mutableListOf<GeneralChatModelUi>()
        val seen = mutableSetOf<String>()
        val local = snapshot()
        if (isComplete(local)) {
            entries.add(GeneralChatModelUi(LOCAL_MODEL_ID, local.model, GeneralChatModelSource.LOCAL))
            seen += LOCAL_MODEL_ID
        }
        accountModels.forEach { model ->
            if (!seen.add(model.modelId)) return@forEach
            entries.add(
                GeneralChatModelUi(
                    id = model.modelId,
                    label = model.modelName.ifEmpty { model.name }.ifEmpty { model.modelId },
                    source = GeneralChatModelSource.ACCOUNT,
                ),
            )
        }
        return entries
    }

    /**
     * Which of them is actually answering — `effectiveSelectedModelId`.
     *
     * A persisted selection wins while that id is still in the live catalog.
     * Otherwise the complete local model wins, then the account's first choice.
     * The account source orders its list with the account primary first, so a
     * phone with no local configuration still follows the choice made elsewhere.
     */
    fun activeModelId(): String = catalog().let { models ->
        val selected = read(SELECTED_MODEL_ID_KEY)
        models.firstOrNull { it.id == selected }?.id
            ?: models.firstOrNull { it.source == GeneralChatModelSource.LOCAL }?.id
            ?: models.firstOrNull()?.id.orEmpty()
    }

    /** @throws SecureStorageUnavailable when the platform keystore refuses. */
    fun selectModel(modelId: String): Boolean {
        if (catalog().none { it.id == modelId }) return false
        try {
            write(SELECTED_MODEL_ID_KEY, modelId)
        } catch (error: SecureStorageUnavailable) {
            throw error
        } catch (error: Throwable) {
            throw SecureStorageUnavailable(error)
        }
        return true
    }

    /**
     * Where the next request goes, or null when nothing can answer it.
     *
     * `activeSnapshot()` and `activeAccessToken()` in one: the endpoint and the
     * credential have to come from the same model, and reading them separately
     * is how a request ends up posting an account key to a local URL.
     */
    fun activeEndpoint(): GeneralChatEndpoint? {
        val active = activeModelId()
        accountModels.firstOrNull { it.modelId == active }?.let { model ->
            return GeneralChatEndpoint(model.apiUrl, model.modelName, model.apiKey)
        }
        val local = snapshot()
        if (!isComplete(local)) return null
        return GeneralChatEndpoint(local.baseUrl, local.model, apiKey())
    }

    /** @throws SecureStorageUnavailable when the platform keystore refuses. */
    fun save(baseUrl: String, model: String, apiKey: String, clearApiKey: Boolean) {
        try {
            write(BASE_URL_KEY, baseUrl.trim().trimEnd('/'))
            write(MODEL_KEY, model.trim())
            when {
                clearApiKey -> secure.delete(API_KEY_KEY)
                apiKey.trim().isNotEmpty() -> write(API_KEY_KEY, apiKey.trim())
            }
        } catch (error: SecureStorageUnavailable) {
            throw error
        } catch (error: Throwable) {
            throw SecureStorageUnavailable(error)
        }
    }

    private fun read(key: String): String = secure.read(key)?.decodeToString().orEmpty()

    private fun write(key: String, value: String) {
        if (value.isEmpty()) secure.delete(key) else secure.write(key, value.encodeToByteArray())
    }

    companion object {
        /** `GENERAL_CHAT_LOCAL_MODEL_ID`; both clients name this row the same. */
        const val LOCAL_MODEL_ID: String = "local-general-chat"

        private const val BASE_URL_KEY = "general_chat_base_url"
        private const val MODEL_KEY = "general_chat_model"
        private const val API_KEY_KEY = "general_chat_api_key"
        private const val SELECTED_MODEL_ID_KEY = "general_chat_selected_model_id"

        private fun isComplete(config: GeneralChatConfigUi): Boolean =
            config.baseUrl.isNotBlank() && config.model.isNotBlank() && config.hasApiKey
    }
}

internal class SecureStorageUnavailable(cause: Throwable?) : RuntimeException(cause)
