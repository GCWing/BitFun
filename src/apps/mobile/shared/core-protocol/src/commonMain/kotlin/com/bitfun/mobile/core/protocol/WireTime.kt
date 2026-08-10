package com.bitfun.mobile.core.protocol

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * Resolves a timestamp from the first usable key in [candidates].
 *
 * The relay has accumulated a long tail of timestamp spellings across desktop
 * versions, and the client cannot know which one a given peer emits. This
 * mirrors `RemoteResponseMapper.firstTimeValue` in the HarmonyOS client exactly:
 * a JSON number becomes its literal text, a string is trimmed, and a blank
 * string falls through to the next candidate rather than winning as `""`.
 *
 * One deliberate difference: an explicit JSON `null` is skipped. The ArkTS
 * version throws a TypeError on that input, because `null` passes its
 * `undefined` check and then reaches `value.trim()`.
 *
 * Returns an empty string when nothing matches, matching the ArkTS default.
 */
internal fun JsonObject.firstWireTime(candidates: List<String>): String {
    for (key in candidates) {
        val element = this[key] ?: continue
        val primitive = element as? JsonPrimitive ?: continue
        if (primitive.content == "null" && !primitive.isString) continue
        val text = if (primitive.isString) primitive.content.trim() else primitive.content
        if (text.isNotEmpty()) return text
    }
    return ""
}

/** Reads a plain string field, treating blank and non-string values as absent. */
internal fun JsonObject.wireString(key: String): String? {
    val primitive = (this[key] as? JsonPrimitive) ?: return null
    if (!primitive.isString) return primitive.content.ifBlank { null }
    return primitive.content.ifBlank { null }
}

/** Reads an integer field, tolerating a numeric string. */
internal fun JsonObject.wireInt(key: String): Int? =
    (this[key] as? JsonPrimitive)?.content?.trim()?.toIntOrNull()

/** Reads a boolean field, tolerating `"true"` / `"false"` as strings. */
internal fun JsonObject.wireBoolean(key: String): Boolean? =
    when ((this[key] as? JsonPrimitive)?.content?.trim()?.lowercase()) {
        "true" -> true
        "false" -> false
        else -> null
    }

/**
 * Timestamp key priority for session entries, in the order
 * `RemoteResponseMapper.sessions` tries them. Order is load-bearing: a peer that
 * sends both `updated_at` and `last_message_at` must resolve to the former.
 */
internal val SESSION_UPDATED_AT_KEYS: List<String> = listOf(
    "updated_at",
    "updatedAt",
    "updated",
    "updated_time",
    "update_time",
    "modified_at",
    "last_active_at",
    "last_activity_at",
    "last_message_at",
    "last_interaction_at",
    "lastUpdated",
    "lastActivityAt",
    "lastMessageAt",
    "timestamp",
)

internal val SESSION_CREATED_AT_KEYS: List<String> = listOf(
    "created_at",
    "createdAt",
    "created",
    "created_time",
    "create_time",
    "inserted_at",
    "started_at",
    "start_time",
)

/** Recent-workspace entries use a shorter list; see `RemoteResponseMapper.recentWorkspaces`. */
internal val RECENT_WORKSPACE_TIME_KEYS: List<String> = listOf(
    "last_opened",
    "lastOpened",
    "updated_at",
    "updatedAt",
)

/** Returns the first non-blank string among [keys], mirroring `a || b` fallbacks. */
internal fun JsonObject.firstWireString(vararg keys: String): String? {
    for (key in keys) {
        wireString(key)?.let { return it }
    }
    return null
}
