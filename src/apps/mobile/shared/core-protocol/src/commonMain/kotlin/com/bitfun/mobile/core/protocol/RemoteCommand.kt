package com.bitfun.mobile.core.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

/** Permission modes the desktop peer accepts for a session. */
@Serializable
public enum class RemotePermissionMode {
    @SerialName("ask")
    Ask,

    @SerialName("auto")
    Auto,

    @SerialName("full_access")
    FullAccess,
}

/**
 * The single outbound command envelope, ported from `RemoteCommand` in
 * `model/RemoteModels.ets`.
 *
 * Every field except [cmd] is optional because one shape serves all ~22
 * commands. `RelayJson` is configured with `explicitNulls = false`, so absent
 * fields are omitted rather than sent as `null` — some desktop handlers
 * distinguish "not supplied" from "supplied as null".
 */
@Serializable
public data class RemoteCommand(
    @SerialName("cmd") val cmd: String,
    @SerialName("_request_id") val requestId: String? = null,
    @SerialName("session_id") val sessionId: String? = null,
    @SerialName("content") val content: String? = null,
    @SerialName("workspace_path") val workspacePath: String? = null,
    @SerialName("path") val path: String? = null,
    @SerialName("agent_type") val agentType: String? = null,
    @SerialName("session_name") val sessionName: String? = null,
    @SerialName("title") val title: String? = null,
    @SerialName("limit") val limit: Int? = null,
    @SerialName("offset") val offset: Int? = null,
    @SerialName("query") val query: String? = null,
    @SerialName("before_message_id") val beforeMessageId: String? = null,
    @SerialName("since_version") val sinceVersion: Int? = null,
    @SerialName("known_msg_count") val knownMessageCount: Int? = null,
    // A [Long] for the same reason [RemoteModelCatalog.version] is: this is that
    // number echoed back, and truncating it here would ask for the catalog on
    // every poll.
    @SerialName("known_model_catalog_version") val knownModelCatalogVersion: Long? = null,
    @SerialName("turn_id") val turnId: String? = null,
    @SerialName("tool_id") val toolId: String? = null,
    @SerialName("model_id") val modelId: String? = null,
    @SerialName("reason") val reason: String? = null,
    @SerialName("mode") val mode: RemotePermissionMode? = null,
    @SerialName("answers") val answers: JsonElement? = null,
    @SerialName("image_contexts") val imageContexts: List<RemoteImageContext>? = null,
    @SerialName("images") val images: List<ImageAttachment>? = null,
)
