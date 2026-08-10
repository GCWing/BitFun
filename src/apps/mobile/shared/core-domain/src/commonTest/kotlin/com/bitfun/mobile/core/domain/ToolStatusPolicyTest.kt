package com.bitfun.mobile.core.domain

import com.bitfun.mobile.core.protocol.RemoteToolStatusResponse
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ToolStatusPolicyTest {
    @Test
    fun bothSpellingsOfWaitingForApprovalCount() {
        assertTrue(ToolStatusPolicy.isPendingConfirmation(tool(status = "pending_confirmation")))
        assertTrue(ToolStatusPolicy.isPendingConfirmation(tool(status = "needs_confirmation")))
        assertFalse(ToolStatusPolicy.isPendingConfirmation(tool(status = "running")))
    }

    @Test
    fun allThreeSpellingsOfFinishedCount() {
        listOf("completed", "done", "sent").forEach { status ->
            assertTrue(ToolStatusPolicy.isCompleted(tool(status = status)), status)
        }
    }

    @Test
    fun aToolThatWroteToStderrFailedEvenIfItSaidItCompleted() {
        assertTrue(ToolStatusPolicy.isFailed(tool(status = "completed", stderr = "no such file")))
        assertFalse(ToolStatusPolicy.isFailed(tool(status = "completed")))
    }

    @Test
    fun aSentQuestionIsStillWaitingOnTheUser() {
        // `sent` means the prompt reached the user, not that they replied — the
        // one place the question vocabulary differs from the finished vocabulary.
        assertTrue(ToolStatusPolicy.isQuestion(tool(name = "AskUserQuestion", status = "sent")))
        assertFalse(ToolStatusPolicy.isQuestion(tool(name = "AskUserQuestion", status = "completed")))
        assertFalse(ToolStatusPolicy.isQuestion(tool(name = "Bash", status = "running")))
    }

    @Test
    fun theQuestionIsPulledOutOfWhicheverShapeTheAgentSent() {
        assertEquals(
            "Overwrite the file?",
            ToolStatusPolicy.questionPrompt(tool(inputPreview = """{"question":"Overwrite the file?"}""")),
        )
        assertEquals(
            "Pick a branch",
            ToolStatusPolicy.questionPrompt(
                tool(inputPreview = """{"questions":[{"header":"Pick a branch"}]}"""),
            ),
        )
    }

    @Test
    fun aPreviewThatIsNotJsonIsTheQuestion() {
        assertEquals("Proceed?", ToolStatusPolicy.questionPrompt(tool(inputPreview = "Proceed?")))
        assertNull(ToolStatusPolicy.questionPrompt(tool()))
    }

    @Test
    fun outputIsCappedSoOneToolCannotFillTheScreen() {
        val output = ToolStatusPolicy.outputText(tool(stdout = "x".repeat(1_000)))
        assertEquals(483, output.length)
        assertTrue(output.endsWith("..."))
    }

    @Test
    fun outputKeepsTheOrderTheCardReadsIn() {
        val output = ToolStatusPolicy.outputText(
            tool(resultPreview = "result", errorPreview = "error", stdout = "out", stderr = "err"),
        )
        assertEquals("result\nerror\nout\nerr", output)
    }
}

private fun tool(
    id: String? = "tool-1",
    name: String? = "Bash",
    status: String? = "running",
    inputPreview: String? = null,
    resultPreview: String? = null,
    errorPreview: String? = null,
    stdout: String? = null,
    stderr: String? = null,
) = RemoteToolStatusResponse(
    id = id,
    name = name,
    status = status,
    inputPreview = inputPreview,
    resultPreview = resultPreview,
    errorPreview = errorPreview,
    stdout = stdout,
    stderr = stderr,
)
