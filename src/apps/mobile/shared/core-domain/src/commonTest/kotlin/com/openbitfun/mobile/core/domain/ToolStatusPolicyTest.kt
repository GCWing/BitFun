package com.openbitfun.mobile.core.domain

import com.openbitfun.mobile.core.protocol.RemoteToolStatusResponse
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
    fun completedVocabularyIncludesSuccessAndLegacyFinishedCaseInsensitively() {
        listOf("completed", "done", "sent", "success", "finished", "Success", "SUCCESS").forEach { status ->
            assertTrue(ToolStatusPolicy.isCompleted(tool(status = status)), status)
        }
        listOf("queued", "unknown").forEach { status ->
            assertFalse(ToolStatusPolicy.isCompleted(tool(status = status)), status)
        }
    }

    @Test
    fun emptyPreviewToolsAreExpandableOnlyForKnownActionableStates() {
        listOf(
            "completed",
            "cancelled",
            "error",
            "failed",
            "running",
            "pending_confirmation",
        ).forEach { status ->
            assertTrue(ToolStatusPolicy.isExpandable(tool(status = status)), status)
        }
        assertTrue(ToolStatusPolicy.isExpandable(tool(name = "AskUserQuestion", status = "sent")))
        assertFalse(ToolStatusPolicy.isExpandable(tool(status = "unknown")))
    }

    @Test
    fun previewFactIsIndependentFromExpandability() {
        assertFalse(ToolStatusPolicy.hasPreview(tool()))
        assertTrue(ToolStatusPolicy.hasPreview(tool(inputPreview = "input")))
        assertTrue(ToolStatusPolicy.hasPreview(tool(resultPreview = "output")))
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
