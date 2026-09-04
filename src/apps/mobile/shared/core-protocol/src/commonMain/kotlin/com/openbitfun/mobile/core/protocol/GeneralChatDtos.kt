package com.openbitfun.mobile.core.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
public data class GeneralChatApiTool(
    @SerialName("id") val id: String,
    @SerialName("name") val name: String,
    @SerialName("status") val status: String,
    @SerialName("input_preview") val inputPreview: String? = null,
    @SerialName("result_preview") val resultPreview: String? = null,
    @SerialName("error_preview") val errorPreview: String? = null,
)

@Serializable
public data class GeneralChatStreamEvent(
    @SerialName("event_id") val eventId: String,
    @SerialName("version") val version: Int,
    @SerialName("session_id") val sessionId: String,
    @SerialName("turn_id") val turnId: String,
    @SerialName("type") val type: String,
    @SerialName("delta") val delta: String? = null,
    @SerialName("tool") val tool: GeneralChatApiTool? = null,
    @SerialName("error_code") val errorCode: String? = null,
    @SerialName("capability") val capability: String? = null,
)
