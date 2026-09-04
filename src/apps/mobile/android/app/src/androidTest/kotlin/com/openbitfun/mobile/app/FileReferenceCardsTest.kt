package com.openbitfun.mobile.app

import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertHeightIsEqualTo
import androidx.compose.ui.test.assertWidthIsEqualTo
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import com.openbitfun.mobile.app.ui.chat.FILE_REFERENCE_CARD_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.FILE_DOWNLOAD_ACTION_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.FileReferenceCards
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import androidx.compose.ui.unit.dp

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

    @Test
    fun downloadIsASeparateActionAndKeepsTheOriginalReference() {
        var downloaded: Pair<String, String>? = null
        var opens = 0

        composeRule.setContent {
            FileReferenceCards(
                text = "See computer:///repo/src/main.kt#L12-40.",
                previewingRemotePath = "",
                previewLoading = false,
                onOpen = { _, _ -> opens += 1 },
                onDownload = { reference, label -> downloaded = reference to label },
                modifier = Modifier,
            )
        }

        composeRule.onNodeWithContentDescription("Download").assertIsDisplayed().performClick()
        composeRule.onNodeWithTag(FILE_DOWNLOAD_ACTION_TEST_TAG)
            .assertWidthIsEqualTo(44.dp)
            .assertHeightIsEqualTo(44.dp)

        assertEquals(0, opens)
        assertEquals("computer:///repo/src/main.kt#L12-40" to "main.kt", downloaded)
    }
}
