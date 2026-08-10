package com.bitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.bitfun.mobile.app.ui.chat.FILE_REFERENCE_CARD_TEST_TAG
import com.bitfun.mobile.app.ui.chat.FileReferenceCards
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

/**
 * The file cards under an agent turn, ported from `MessageFileCards`.
 *
 * The projection itself is covered in `core-feature`; what is pinned here is the
 * wiring — that a card carries the verbatim reference to whatever opens the
 * preview, and that the card for the open file is the one that spins.
 */
class FileReferenceCardsTest {
    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun everyFileNamedInATurnGetsACardNamedByItsFile() {
        composeRule.setContent {
            FileReferenceCards(
                text = "Look at [it](computer:///repo/README.md) and computer:///repo/src/main.kt.",
                previewingRemotePath = "",
                previewLoading = false,
                onOpen = { _, _ -> },
                modifier = Modifier,
            )
        }

        assertEquals(2, composeRule.onAllNodesWithTag(FILE_REFERENCE_CARD_TEST_TAG).fetchSemanticsNodes().size)
        composeRule.onNodeWithText("README.md").assertIsDisplayed()
        composeRule.onNodeWithText("main.kt").assertIsDisplayed()
    }

    @Test
    fun tappingACardReportsTheReferenceAsTheAgentWroteIt() {
        var opened: Pair<String, String>? = null

        composeRule.setContent {
            FileReferenceCards(
                // With a line marker, because that is what the preview needs in
                // order to scroll to the line the agent was talking about.
                text = "See computer:///repo/src/main.kt#L12-40 for the cause.",
                previewingRemotePath = "",
                previewLoading = false,
                onOpen = { reference, label -> opened = reference to label },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithText("main.kt").performClick()

        assertEquals("computer:///repo/src/main.kt#L12-40" to "main.kt", opened)
    }

    @Test
    fun aTurnThatNamesNoFileDrawsNothing() {
        composeRule.setContent {
            FileReferenceCards(
                text = "Run `src/main.kt` and read https://example.com/README.md.",
                previewingRemotePath = "",
                previewLoading = false,
                onOpen = { _, _ -> },
                modifier = Modifier,
            )
        }

        assertEquals(0, composeRule.onAllNodesWithTag(FILE_REFERENCE_CARD_TEST_TAG).fetchSemanticsNodes().size)
    }

    @Test
    fun theOpenFileIsStillTappableWhileItIsLoading() {
        var opens = 0

        composeRule.setContent {
            FileReferenceCards(
                text = "computer:///repo/README.md",
                previewingRemotePath = "/repo/README.md",
                previewLoading = true,
                onOpen = { _, _ -> opens += 1 },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithText("README.md").assertIsDisplayed()
        composeRule.onNodeWithText("README.md").performClick()

        assertEquals(1, opens)
    }
}
