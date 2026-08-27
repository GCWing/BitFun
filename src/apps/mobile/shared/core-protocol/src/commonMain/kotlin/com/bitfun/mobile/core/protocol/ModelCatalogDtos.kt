package com.bitfun.mobile.core.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive

@Serializable
public data class RemoteReasoningPresetAction(
    @SerialName("type") val type: String,
    /** The peer sends either a string or a number here; both are kept verbatim. */
    @SerialName("value") val value: JsonPrimitive? = null,
    @SerialName("enabled") val enabled: Boolean? = null,
    @SerialName("body") val body: JsonElement? = null,
)

@Serializable
public data class RemoteReasoningPresetDescriptor(
    @SerialName("id") val id: String,
    @SerialName("label") val label: String,
    @SerialName("order") val order: Int,
    @SerialName("actions") val actions: List<RemoteReasoningPresetAction> = emptyList(),
    @SerialName("source") val source: String,
)

@Serializable
public data class RemoteReasoningCatalogProjection(
    @SerialName("status") val status: String,
    @SerialName("default_preset") val defaultPreset: String? = null,
    @SerialName("presets") val presets: List<RemoteReasoningPresetDescriptor> = emptyList(),
)

@Serializable
public data class RemoteModelConfig(
    @SerialName("id") val id: String = "",
    @SerialName("name") val name: String = "",
    @SerialName("provider") val provider: String = "",
    @SerialName("base_url") val baseUrl: String = "",
    @SerialName("model_name") val modelName: String = "",
    @SerialName("context_window") val contextWindow: Int? = null,
    @SerialName("enabled") val enabled: Boolean = false,
    @SerialName("capabilities") val capabilities: List<String> = emptyList(),
    @SerialName("reasoning") val reasoning: RemoteReasoningCatalogProjection? = null,
)

@Serializable
public data class RemoteDefaultModels(
    @SerialName("primary") val primary: String? = null,
    @SerialName("fast") val fast: String? = null,
    @SerialName("search") val search: String? = null,
    @SerialName("image_understanding") val imageUnderstanding: String? = null,
)

/**
 * [version] lets the client send `known_model_catalog_version` and skip the
 * payload when nothing changed, so this arrives rarely despite its size. Legacy
 * or minimal peers may omit `version`; the default keeps the catalog decodable
 * instead of failing the whole reply as it did when the field was required.
 *
 * It is a [Long] rather than an [Int] because the desktop derives it from the
 * catalog's last-modified time in milliseconds and masks it to 53 bits — an
 * epoch-millisecond value passes 2^31 and stays there, so every real desktop
 * sends a number an [Int] cannot hold.
 */
@Serializable
public data class RemoteModelCatalog(
    @SerialName("version") val version: Long = 0L,
    @SerialName("models") val models: List<RemoteModelConfig> = emptyList(),
    @SerialName("default_models") val defaultModels: RemoteDefaultModels = RemoteDefaultModels(),
    @SerialName("session_model_id") val sessionModelId: String? = null,
)

@Serializable
public data class ModelCatalogResponse(
    @SerialName("resp") override val resp: String? = null,
    @SerialName("message") override val message: String? = null,
    @SerialName("catalog") val catalog: RemoteModelCatalog? = null,
) : CommandStatus

@Serializable
public data class SetSessionModelResponse(
    @SerialName("resp") override val resp: String? = null,
    @SerialName("message") override val message: String? = null,
    @SerialName("session_id") val sessionId: String? = null,
    @SerialName("model_id") val modelId: String? = null,
) : CommandStatus
