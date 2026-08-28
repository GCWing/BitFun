package com.bitfun.mobile.core.domain

import com.bitfun.mobile.core.protocol.RelayJson
import com.bitfun.mobile.core.protocol.RemoteToolStatusResponse
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals

class ToolQuestionPolicyTest {
    @Test
    fun parsesStructuredQuestionsFromToolInput() {
        val tool = tool(
            toolInput = RelayJson.parseToJsonElement(
                """{"questions":[{"header":"Branch","question":"Which branch?","options":[{"label":"main","description":"Stable"},{"label":"dev"}],"multiSelect":false},{"question":"Checks","options":[{"label":"lint"},{"label":"tests"}],"multiSelect":true}]}""",
            ),
        )

        val questions = ToolQuestionPolicy.parse(tool)

        assertEquals(2, questions.size)
        assertEquals(0, questions[0].index)
        assertEquals("Branch", questions[0].header)
        assertEquals("Which branch?", questions[0].question)
        assertEquals(listOf(QuestionOptionSpec("main", "Stable"), QuestionOptionSpec("dev", null)), questions[0].options)
        assertEquals(false, questions[0].multiSelect)
        assertEquals(1, questions[1].index)
        assertEquals(true, questions[1].multiSelect)
    }

    @Test
    fun parsesStringifiedInputPreviewWhenToolInputIsAbsent() {
        val questions = ToolQuestionPolicy.parse(
            tool(inputPreview = """{"questions":[{"header":"Pick","question":"Choose","options":[{"label":"one"}]}]}"""),
        )

        assertEquals(listOf(QuestionSpec(0, "Pick", "Choose", listOf(QuestionOptionSpec("one", null)), false)), questions)
    }

    @Test
    fun dropsHeaderOnlyQuestionsAndPreservesOriginalIndexAfterDroppedEntry() {
        val questions = ToolQuestionPolicy.parse(
            tool(
                toolInput = RelayJson.parseToJsonElement(
                    """{"questions":[{"header":"Missing question","options":[{"label":"ignored"}]},{"question":"Valid","options":[{"label":"keep"}]}]}""",
                ),
            ),
        )

        assertEquals(listOf(QuestionSpec(1, "", "Valid", listOf(QuestionOptionSpec("keep", null)), false)), questions)
    }

    @Test
    fun malformedToolInputFallsBackToInputPreview() {
        val questions = ToolQuestionPolicy.parse(
            tool(
                toolInput = JsonPrimitive("not valid json"),
                inputPreview = """{"questions":[{"question":"Fallback","options":[{"label":"yes"}]}]}""",
            ),
        )

        assertEquals(listOf(QuestionSpec(0, "", "Fallback", listOf(QuestionOptionSpec("yes", null)), false)), questions)
    }

    @Test
    fun dropsQuestionsWithoutOptionsAndLegacyShapes() {
        assertEquals(
            emptyList(),
            ToolQuestionPolicy.parse(tool(toolInput = RelayJson.parseToJsonElement("""{"questions":[{"question":"No choices","options":[]}]}"""))),
        )
        assertEquals(emptyList(), ToolQuestionPolicy.parse(tool(inputPreview = """{"question":"Old"}""")))
        assertEquals(emptyList(), ToolQuestionPolicy.parse(tool(inputPreview = """{"prompt":"Old"}""")))
        assertEquals(emptyList(), ToolQuestionPolicy.parse(tool(inputPreview = "{}")))
    }

    private fun tool(
        toolInput: JsonElement? = null,
        inputPreview: String? = null,
    ): RemoteToolStatusResponse = RemoteToolStatusResponse(
        name = "AskUserQuestion",
        status = "sent",
        toolInput = toolInput,
        inputPreview = inputPreview,
    )
}
