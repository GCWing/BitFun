package com.openbitfun.mobile.core.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

@Serializable
public data class RemoteToolStatusResponse(
    @SerialName("id") val id: String? = null,
    @SerialName("name") val name: String? = null,
    @SerialName("status") val status: String? = null,
    @SerialName("duration_ms") val durationMs: Long? = null,
    @SerialName("start_ms") val startMs: Long? = null,
    @SerialName("input_preview") val inputPreview: String? = null,
    @SerialName("tool_input") val toolInput: JsonElement? = null,
    @SerialName("stdout") val stdout: String? = null,
    @SerialName("stderr") val stderr: String? = null,
    @SerialName("tool_output") val toolOutput: JsonElement? = null,
    @SerialName("result_preview") val resultPreview: String? = null,
    @SerialName("error_preview") val errorPreview: String? = null,
    @SerialName("exit_code") val exitCode: Int? = null,
)

@Serializable
public data class ChatMessageItemResponse(
    @SerialName("type") val type: String? = null,
    @SerialName("content") val content: String? = null,
    @SerialName("tool") val tool: RemoteToolStatusResponse? = null,
    @SerialName("is_subagent") val isSubagent: Boolean? = null,
    // The peer spells this one in camelCase; it is not a typo on this side.
    @SerialName("subItems") val subItems: List<ChatMessageItemResponse>? = null,
)

@Serializable
public data class ImageAttachment(
    @SerialName("name") val name: String,
    @SerialName("data_url") val dataUrl: String,
)

@Serializable
public data class RemoteImageContext(
    @SerialName("id") val id: String,
    @SerialName("image_path") val imagePath: String? = null,
    @SerialName("data_url") val dataUrl: String? = null,
    @SerialName("mime_type") val mimeType: String,
    @SerialName("metadata") val metadata: JsonElement? = null,
)

@Serializable
public data class ChatMessageResponse(
    @SerialName("id") val id: String? = null,
    @SerialName("message_id") val messageId: String? = null,
    @SerialName("role") val role: String,
    @SerialName("content") val content: String,
    @SerialName("timestamp") val timestamp: String? = null,
    @SerialName("metadata") val metadata: JsonElement? = null,
    @SerialName("thinking") val thinking: String? = null,
    @SerialName("items") val items: List<ChatMessageItemResponse> = emptyList(),
    @SerialName("tools") val tools: List<RemoteToolStatusResponse> = emptyList(),
    @SerialName("images") val images: List<ImageAttachment> = emptyList(),
) {
    /** `id` and `message_id` are the same field under two spellings. */
    public val resolvedId: String? get() = id ?: messageId
}

@Serializable
public data class ActiveTurnSnapshotResponse(
    @SerialName("turn_id") val turnId: String,
    @SerialName("status") val status: String,
    @SerialName("text") val text: String? = null,
    @SerialName("thinking") val thinking: String? = null,
    @SerialName("tools") val tools: List<RemoteToolStatusResponse> = emptyList(),
    @SerialName("round_index") val roundIndex: Int? = null,
    @SerialName("items") val items: List<ChatMessageItemResponse> = emptyList(),
)

@Serializable
public data class SessionMessagesResponse(
    @SerialName("resp") override val resp: String? = null,
    @SerialName("message") override val message: String? = null,
    @SerialName("messages") val messages: List<ChatMessageResponse> = emptyList(),
    @SerialName("has_more") val hasMore: Boolean = false,
) : CommandStatus

/**
 * The cursor-based poll reply that drives remote chat.
 *
 * [version] is the cursor: the client sends it back as `since_version` and the
 * peer answers with [changed] = false when nothing moved, which is the common
 * case and must stay cheap to decode.
 */
@Serializable
public data class PollSessionResponse(
    @SerialName("resp") override val resp: String? = null,
    @SerialName("message") override val message: String? = null,
    @SerialName("version") val version: Int = 0,
    @SerialName("changed") val changed: Boolean = false,
    @SerialName("session_state") val sessionState: String? = null,
    @SerialName("title") val title: String? = null,
    @SerialName("new_messages") val newMessages: List<ChatMessageResponse> = emptyList(),
    @SerialName("total_msg_count") val totalMessageCount: Int? = null,
    @SerialName("active_turn") val activeTurn: ActiveTurnSnapshotResponse? = null,
    @SerialName("model_catalog") val modelCatalog: RemoteModelCatalog? = null,
) : CommandStatus
