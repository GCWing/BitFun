package com.openbitfun.mobile.core.domain

import com.openbitfun.mobile.core.protocol.RelayJson
import com.openbitfun.mobile.core.protocol.RemoteToolStatusResponse
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonPrimitive

public data class QuestionOptionSpec public constructor(
    public val label: String,
    public val description: String?,
)

public data class QuestionSpec public constructor(
    public val index: Int,
    public val header: String,
    public val question: String,
    public val options: List<QuestionOptionSpec>,
    public val multiSelect: Boolean,
)

public object ToolQuestionPolicy {
    public fun parse(tool: RemoteToolStatusResponse): List<QuestionSpec> {
        val candidates = listOfNotNull(
            tool.toolInput?.let(::asObject),
            tool.inputPreview?.takeIf(String::isNotBlank)?.let { preview ->
                runCatching { RelayJson.parseToJsonElement(preview) }.getOrNull()?.let(::asObject)
            },
        )
        return candidates.firstNotNullOfOrNull { root -> parseQuestions(root).takeIf(List<QuestionSpec>::isNotEmpty) }
            ?: emptyList()
    }

    private fun asObject(element: JsonElement): JsonObject? {
        val candidate = if (element is JsonPrimitive && element.isString) {
            runCatching { RelayJson.parseToJsonElement(element.content) }.getOrNull()
        } else {
            element
        }
        return candidate as? JsonObject
    }

    private fun parseQuestions(root: JsonObject): List<QuestionSpec> =
        (root["questions"] as? JsonArray).orEmpty().mapIndexedNotNull { index, element ->
            val question = element as? JsonObject ?: return@mapIndexedNotNull null
            val prompt = question.text("question") ?: return@mapIndexedNotNull null
            val options = (question["options"] as? JsonArray).orEmpty().mapNotNull { optionElement ->
                val option = optionElement as? JsonObject ?: return@mapNotNull null
                val label = option.text("label") ?: return@mapNotNull null
                QuestionOptionSpec(label, option.text("description"))
            }
            if (options.isEmpty()) return@mapIndexedNotNull null
            QuestionSpec(
                index = index,
                header = question.text("header").orEmpty(),
                question = prompt,
                options = options,
                multiSelect = (question["multiSelect"] as? JsonPrimitive)?.booleanOrNull ?: false,
            )
        }

    private fun JsonObject.text(key: String): String? =
        (this[key] as? JsonPrimitive)?.takeIf { it.isString }?.jsonPrimitive?.content
            ?.trim()
            ?.takeIf(String::isNotEmpty)
}
